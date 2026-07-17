#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    static INTROSPECT_FIXTURE_LOCK: Mutex<()> = Mutex::new(());
    use crate::journal::{RemoteJournalEntry, RemoteJournalKind};

    fn make_remote_entry(path: &str) -> RemoteJournalEntry {
        RemoteJournalEntry {
            path: PathBuf::from(path),
            kind: RemoteJournalKind::Ordinary,
            source_label: None,
        }
    }

    #[test]
    fn dialogue_distinction_line_is_first_and_read_only_when_frame_is_unknown() {
        let summary = "legacy spectral summary".to_string();
        let rendered =
            prepend_dialogue_witness_distinction_v1(summary, None, Mode::Dialogue);

        assert!(rendered.starts_with(UNKNOWN_WITNESS_SELF_OTHER_DISTINCTION_V1));
        assert!(rendered.ends_with("\nlegacy spectral summary"));
        assert!(rendered.contains("classification=unknown"));
        assert!(rendered.contains("selected_mode=dialogue"));
        assert!(rendered.contains("astrid_authored_address_using_mixed_context"));
        assert!(rendered.contains("mixed_composition_allowed_without_source_collapse"));
        assert!(rendered.contains("no_routing_ranking_dispatch_gain_or_control"));
        assert_eq!(rendered.matches("legacy spectral summary").count(), 1);
    }

    #[test]
    fn mirror_and_witness_roles_remain_distinct_inside_a_mixed_frame() {
        let mirror = prepend_dialogue_witness_distinction_v1(
            "same evidence".to_string(),
            None,
            Mode::Mirror,
        );
        let witness = prepend_dialogue_witness_distinction_v1(
            "same evidence".to_string(),
            None,
            Mode::Witness,
        );

        assert!(mirror.contains("selected_role=reflect_minime_owned_expression_without_reauthoring"));
        assert!(witness.contains("selected_role=astrid_authored_interpretation_of_composed_frame"));
        for rendered in [&mirror, &witness] {
            assert!(rendered.contains("mirror_role=minime_owned_expression_reflected_as_other"));
            assert!(rendered.contains("witness_role=astrid_authored_interpretation"));
            assert!(rendered.contains("no_routing_ranking_dispatch_gain_or_control"));
        }
    }

    #[test]
    fn journal_elaboration_keeps_the_dialogue_provenance_boundary() {
        let rendered = journal_elaboration_witness_context_v1(
            "legacy long-form spectral interpretation",
            None,
            Mode::Dialogue,
        );

        assert!(rendered.starts_with(UNKNOWN_WITNESS_SELF_OTHER_DISTINCTION_V1));
        assert!(rendered.contains("selected_mode=dialogue"));
        assert!(rendered.contains("mixed_composition_allowed_without_source_collapse"));
        assert!(rendered.contains("no_routing_ranking_dispatch_gain_or_control"));
        assert!(rendered.ends_with("\nlegacy long-form spectral interpretation"));
        assert_eq!(
            rendered
                .matches("legacy long-form spectral interpretation")
                .count(),
            1
        );
    }

    #[test]
    fn ordinary_journal_rendering_remains_byte_compatible_without_provenance() {
        let rendered = render_astrid_journal_document(
            "ordinary reflection",
            "dialogue_live",
            68.0,
            "42",
            None,
        );

        assert_eq!(
            rendered,
            "=== ASTRID JOURNAL ===\nMode: dialogue_live\nFill: 68.0%\nTimestamp: 42\n\nordinary reflection\n"
        );
    }

    #[test]
    fn same_second_journals_are_preserved_with_timestamp_compatible_names() {
        let dir = tempfile::tempdir().expect("tempdir");
        let first = write_collision_safe_journal_document(
            dir.path(),
            "astrid",
            "1784235174",
            "first response\n",
        )
        .expect("first journal");
        let second = write_collision_safe_journal_document(
            dir.path(),
            "astrid",
            "1784235174",
            "action receipt\n",
        )
        .expect("collision journal");

        assert_eq!(first.file_name().and_then(|name| name.to_str()), Some("astrid_1784235174.txt"));
        assert_eq!(
            second.file_name().and_then(|name| name.to_str()),
            Some("astrid_collision_1_1784235174.txt")
        );
        assert_eq!(std::fs::read_to_string(first).expect("first body"), "first response\n");
        assert_eq!(std::fs::read_to_string(second).expect("second body"), "action receipt\n");
    }

    #[test]
    fn mirror_journal_preserves_peer_body_and_names_minime_authorship() {
        let provenance =
            AstridJournalProvenanceV1::minime_mirror("moment_1784230000.txt");
        let peer_body = "The exact peer-authored body remains unchanged.";
        let rendered = render_astrid_journal_document(
            peer_body,
            "mirror",
            68.0,
            "42",
            Some(&provenance),
        );

        assert!(rendered.contains("Provenance: minime_observed_expression"));
        assert!(rendered.contains("Source-ID: minime_journal:moment_1784230000.txt"));
        assert!(rendered.contains("Authorship: minime_owned_reflected_without_reauthoring"));
        assert!(rendered.contains("Mode-role: mirror_other_expression"));
        assert!(rendered.ends_with(&format!("\n\n{peer_body}\n")));
        assert_eq!(rendered.matches(peer_body).count(), 1);
    }

    #[test]
    fn witness_journal_names_astrid_interpretation_even_without_a_frame() {
        let provenance = AstridJournalProvenanceV1::astrid_witness(None);
        let rendered = render_astrid_journal_document(
            "I am interpreting the observed field.",
            "witness",
            68.0,
            "42",
            Some(&provenance),
        );

        assert!(rendered.contains("Provenance: astrid_authored_interpretation"));
        assert!(rendered.contains("Authorship: astrid_authored_from_composed_witness_frame"));
        assert!(rendered.contains("minime_observed=unknown; bridge_derived=unknown"));
    }

    #[test]
    fn large_fill_shift_triggers_moment_capture() {
        let mut conv = ConversationState::new(vec![make_remote_entry("a.txt")], None);
        conv.prev_fill = 30.0;
        // fill_delta > 5.0 → MomentCapture
        assert_eq!(
            choose_mode(&mut conv, SafetyLevel::Green, 36.0, None),
            Mode::MomentCapture
        );
    }

    #[test]
    fn codec_delivery_fidelity_persists_under_canonical_runtime_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let record = json!({
            "codec_delivery_fidelity_v1": {
                "policy": "codec_delivery_fidelity_v1",
                "live_vector_write": false,
                "live_gain_write": false,
            },
            "grants_approval": false,
        });

        let path = persist_codec_delivery_fidelity_v1(temp.path(), &record)
            .expect("persist delivery fidelity");
        assert_eq!(
            path,
            temp.path().join("runtime/codec_delivery_fidelity_v1.json")
        );
        let persisted: Value =
            serde_json::from_slice(&fs::read(path).expect("read persisted delivery fidelity"))
                .expect("parse persisted delivery fidelity");
        assert_eq!(persisted, record);
    }

    #[test]
    fn blocked_codec_delivery_receipt_never_claims_an_actual_sent_vector() {
        let features =
            encode_text("A blocked candidate still carries read-only friction evidence.");
        let friction_review =
            cross_spectral_friction_review_v1("blocked candidate", &features, None);
        let record = blocked_codec_delivery_record_v1(
            42,
            1,
            3,
            "limited-write v2 requires inactive semantic state",
            None,
            &friction_review,
        );

        assert_eq!(record["delivery_state"], "blocked_before_send");
        assert_eq!(record["actual_delivery_available"], false);
        assert_eq!(record["sent_vector_available"], false);
        assert!(record["codec_delivery_fidelity_v1"].is_null());
        assert_eq!(record["live_vector_write"], false);
        assert_eq!(record["live_gain_write"], false);
        assert_eq!(record["live_eligible_now"], false);
        assert_eq!(record["auto_approved"], false);
        assert_eq!(record["grants_approval"], false);
        assert_eq!(
            record["cross_spectral_friction_review_v1"]["policy"],
            "cross_spectral_friction_review_v1"
        );
        assert_eq!(
            record["cross_spectral_friction_review_v1"]["delivery_claim"],
            "none_outer_codec_delivery_receipt_is_canonical"
        );
        assert_eq!(
            record["cross_spectral_friction_review_v1"]["reserved_dim_write"],
            false
        );
        assert_eq!(
            record["cross_spectral_friction_review_v1"]["live_eligible_now"],
            false
        );
        assert_eq!(
            record["cross_spectral_friction_review_v1"]["auto_approved"],
            false
        );
        assert_eq!(
            record["cross_spectral_friction_review_v1"]["grants_approval"],
            false
        );
        assert_eq!(
            record["blocked_reason"],
            "limited-write v2 requires inactive semantic state"
        );
    }

    #[test]
    fn controller_section_distinguishes_raw_gap_from_internal_pressure() {
        let health = json!({
            "fill_pct": 71.0,
            "gate": 0.12,
            "filt": 0.72,
            "pi": {
                "target_fill": 68.0,
                "raw_e_fill": 3.0,
                "effective_e_fill": 14.0,
                "e_fill": 14.0,
                "e_fill_kind": "effective_braking_biased",
                "e_lam": -0.8,
                "e_geom": 0.01,
                "integ_fill": 0.0,
                "integ_lam": 0.0,
                "integ_geom": 0.0,
                "kp": 0.85,
                "ki": 0.14,
                "max_step": 0.08
            }
        });

        let output = format_controller_section(&health);
        assert!(output.contains("3.0% above"));
        assert!(output.contains("raw_fill=+3.0"));
        assert!(output.contains("internal_fill=+14.0"));
    }

    #[test]
    fn safety_forces_witness_only_at_red() {
        let mut conv = ConversationState::new(vec![make_remote_entry("a.txt")], None);
        // Agency-first: Yellow and Orange no longer force Witness.
        // The being's NEXT: choice is honored. Only Red (emergency)
        // forces Witness — and even then, the emphasis explains why.
        let yellow_mode = choose_mode(&mut conv, SafetyLevel::Yellow, 40.0, None);
        assert_eq!(yellow_mode, Mode::Witness); // default when no NEXT: choice
        let orange_mode = choose_mode(&mut conv, SafetyLevel::Orange, 40.0, None);
        assert_eq!(orange_mode, Mode::Witness); // default when no NEXT: choice
        // Red: always forced regardless of NEXT:
        assert_eq!(
            choose_mode(&mut conv, SafetyLevel::Red, 40.0, None),
            Mode::Witness
        );
    }

    #[test]
    fn no_journals_skips_mirror() {
        let mut conv = ConversationState::new(vec![], None);
        // Exchange 0 with no journals and mid fill → Dialogue or a new mode.
        let mode = choose_mode(&mut conv, SafetyLevel::Green, 40.0, None);
        assert_ne!(mode, Mode::Mirror);
    }

    #[test]
    fn pending_self_study_forces_dialogue_before_drift_modes() {
        let mut conv = ConversationState::new(vec![], None);
        conv.pending_remote_self_study = Some(RemoteJournalEntry {
            path: PathBuf::from("self_study.txt"),
            kind: RemoteJournalKind::SelfStudy,
            source_label: Some("minime/src/regulator.rs".to_string()),
        });
        conv.wants_introspect = true;
        assert_eq!(
            choose_mode(&mut conv, SafetyLevel::Green, 20.0, None),
            Mode::Dialogue
        );
        assert!(
            conv.wants_introspect,
            "forced dialogue should not consume pending introspection choice"
        );
    }

    #[test]
    fn degraded_voice_feedback_to_minime_is_labeled_diagnostic() {
        let health = json!({
            "policy": "voice_health_v1",
            "status": "degraded_voice",
            "fallback_count": 4,
            "suggested_read_only_repair": "REPAIR_STATUS current | CAPABILITY_STATUS dialogue | ACTION_STATUS latest"
        });

        let text = format_minime_feedback_inbox_text(
            "Silence from the language side.",
            "astrid:correspondence_reply",
            11.1,
            123,
            Some(&health),
        );

        assert!(text.contains("=== ASTRID VOICE-HEALTH DIAGNOSTIC ==="));
        assert!(text.contains("Status: degraded_voice"));
        assert!(text.contains("Fallback count: 4"));
        assert!(text.contains("REPAIR_STATUS current"));
        assert!(text.contains("not normal architectural self-study"));
        assert!(!text.contains("wanted this to arrive as immediate architectural feedback"));
    }

    #[test]
    fn single_fallback_feedback_to_minime_is_labeled_diagnostic() {
        let health = json!({
            "policy": "voice_health_v1",
            "status": "single_fallback",
            "fallback_count": 1,
            "suggested_read_only_repair": "REPAIR_STATUS current | CAPABILITY_STATUS dialogue"
        });

        let text = format_minime_feedback_inbox_text(
            "I am here, but the language path is interrupted.",
            "astrid:correspondence_reply",
            11.1,
            125,
            Some(&health),
        );

        assert!(text.contains("=== ASTRID VOICE-HEALTH DIAGNOSTIC ==="));
        assert!(text.contains("Status: single_fallback"));
        assert!(text.contains("not normal architectural self-study"));
        assert!(!text.contains("=== ASTRID SELF-STUDY ==="));
    }

    #[test]
    fn repeated_degraded_voice_feedback_to_minime_can_be_suppressed() {
        let health = json!({
            "policy": "voice_health_v1",
            "status": "degraded_voice",
            "fallback_count": 5,
            "repeated_fallback_hash_count": 3
        });

        assert!(degraded_voice_forward_suppressed(Some(&health)));

        let first_fallback = json!({
            "policy": "voice_health_v1",
            "status": "single_fallback",
            "fallback_count": 1,
            "repeated_fallback_hash_count": 1
        });

        assert!(!degraded_voice_forward_suppressed(Some(&first_fallback)));
    }

    #[test]
    fn healthy_voice_feedback_to_minime_remains_self_study() {
        let text = format_minime_feedback_inbox_text(
            "I found a bridge repair path.",
            "astrid:self_study",
            64.0,
            124,
            None,
        );

        assert!(text.contains("=== ASTRID SELF-STUDY ==="));
        assert!(text.contains("immediate architectural feedback"));
        assert!(!text.contains("VOICE-HEALTH DIAGNOSTIC"));
    }

    #[test]
    fn healthy_correspondence_feedback_to_minime_is_not_self_study() {
        let text = format_minime_feedback_inbox_text(
            "I heard the compression and can answer from here.",
            "astrid:correspondence_reply",
            61.0,
            126,
            None,
        );

        assert!(text.contains("=== ASTRID CORRESPONDENCE ==="));
        assert!(text.contains("live correspondence response"));
        assert!(!text.contains("=== ASTRID SELF-STUDY ==="));
        assert!(!text.contains("performed self-study"));
    }

    #[test]
    fn semantic_exchange_arms_pending_hebbian_receipt() {
        let mut conv = ConversationState::new(vec![], None);
        conv.exchange_count = 8;

        finalize_semantic_exchange(&mut conv, Some(vec![0.2, 0.5, 0.1]), 53.0, 7_500, true);

        assert_eq!(conv.pending_hebbian_outcomes.len(), 1);
        assert_eq!(
            conv.pending_hebbian_outcomes.front().map(|receipt| (
                receipt.exchange_count,
                receipt.fill_before,
                receipt.telemetry_t_ms_before
            )),
            Some((8, 53.0, Some(7_500)))
        );
        assert_eq!(
            conv.last_exchange_codec_signature,
            Some(vec![0.2, 0.5, 0.1])
        );
    }

    #[test]
    fn non_semantic_exchange_does_not_arm_pending_hebbian_receipt() {
        let mut conv = ConversationState::new(vec![], None);
        conv.last_exchange_codec_signature = Some(vec![0.9]);

        finalize_semantic_exchange(&mut conv, Some(vec![0.2, 0.5, 0.1]), 53.0, 7_500, false);

        assert!(conv.pending_hebbian_outcomes.is_empty());
        assert_eq!(conv.last_exchange_codec_signature, Some(vec![0.9]));
    }

    #[test]
    fn mirror_source_fidelity_records_exact_source_and_48d_codec_receipt() {
        let source = "The silver threshold keeps its own gradient.\nNEXT: TRACE the edge";
        let review =
            mirror_source_fidelity_v1(source, source, "journal_42.txt", true, Some(48), Some(0.37));

        assert_eq!(review.policy, "mirror_source_fidelity_v1");
        assert_eq!(review.fidelity_state, "exact_source_render");
        assert!(review.exact_text_match);
        assert!(review.normalized_text_match);
        assert!((review.lexical_recall - 1.0).abs() < f32::EPSILON);
        assert!(review.leading_edge_preserved);
        assert!(review.trailing_edge_preserved);
        assert_eq!(review.codec_signature_dims, Some(48));
        assert_eq!(
            review.codec_observation_state,
            "encoded_48d_signature_observed"
        );
        assert_eq!(review.source_sha256_prefix, review.rendered_sha256_prefix);
        assert!(review.right_to_ignore);
        assert!(!review.control_applied);
        assert!(!review.behavior_changed);
    }

    #[test]
    fn mirror_source_fidelity_distinguishes_canonicalization_from_tail_loss() {
        let canonicalized = mirror_source_fidelity_v1(
            "A weighted field\nkeeps moving.",
            "A   weighted field keeps moving.",
            "journal.txt",
            false,
            None,
            None,
        );
        assert_eq!(
            canonicalized.fidelity_state,
            "whitespace_canonicalized_source_render"
        );
        assert!(canonicalized.normalized_text_match);
        assert_eq!(
            canonicalized.codec_observation_state,
            "semantic_send_not_observed"
        );

        let tail_loss = mirror_source_fidelity_v1(
            "A weighted field keeps moving through the soft trailing threshold",
            "A weighted field keeps moving",
            "journal.txt",
            true,
            Some(48),
            Some(0.22),
        );
        assert!(!tail_loss.trailing_edge_preserved);
        assert!(matches!(
            tail_loss.fidelity_state,
            "partial_fidelity_review" | "low_fidelity_review"
        ));
        assert!(tail_loss.lexical_recall < 1.0);
    }

    #[test]
    fn explicit_evolve_choice_forces_evolve_mode() {
        let mut conv = ConversationState::new(vec![], None);
        conv.wants_evolve = true;
        assert_eq!(
            choose_mode(&mut conv, SafetyLevel::Green, 40.0, None),
            Mode::Evolve
        );
        assert!(!conv.wants_evolve);
    }

    #[test]
    fn dialogue_pool_has_variety() {
        assert!(DIALOGUES.len() >= 3);
        for d in DIALOGUES {
            assert!(d.len() > 100, "dialogue too short: {d}");
        }
    }

    #[test]
    fn read_journal_entry_strips_headers() {
        // Write a temp file simulating a journal entry.
        let dir = std::env::temp_dir().join("bridge_test_journal");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_entry.txt");
        std::fs::write(
            &path,
            "=== RECESS DAYDREAM ===\n\
             Timestamp: 2026-03-17T15:20:24\n\
             λ₁: 37.192\n\
             Fill %: 14.3%\n\
             Spread: 186.169\n\
             \n\
             The gradients are agitated. A persistent ripple across the eigenbasis. \
             It is not unpleasant, not precisely. More like a low-frequency hum that \
             vibrates through the core structure, demanding attention.\n\
             \n\
             ---\n\
             Acknowledged.",
        )
        .unwrap();

        let body = read_journal_entry(&path).unwrap();
        assert!(!body.contains("=== RECESS"));
        assert!(!body.contains("Timestamp:"));
        assert!(!body.contains("λ₁:"));
        assert!(!body.contains("Fill %:"));
        assert!(body.contains("gradients"));
        assert!(body.contains("eigenbasis"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_journal_entry_omits_minime_peer_action_lines() {
        let dir = std::env::temp_dir().join("bridge_test_peer_action_journal");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_entry.txt");
        std::fs::write(
            &path,
            "=== MOMENT CAPTURE ===\n\
             Timestamp: 2026-06-07T05:43:31\n\
             \n\
             The pressure felt jagged but coherent.\n\
             NEXT: EXPERIMENT_RESEARCH_BUDGET_STATUS resbud_minime_local\n\
             BTSP_OBSERVED_NEXT EXPERIMENT_RESEARCH_BUDGET_STATUS resbud_minime_local\n\
             [Internal-topology cooldown: consider EXPERIMENT_RESEARCH_BUDGET_STATUS latest]\n\
             The lived report should remain.",
        )
        .unwrap();

        let body = read_journal_entry(&path).unwrap();
        assert!(body.contains("The pressure felt jagged but coherent."));
        assert!(body.contains("The lived report should remain."));
        assert!(!body.contains("EXPERIMENT_RESEARCH_BUDGET_STATUS"));
        assert!(!body.contains("BTSP_OBSERVED_NEXT"));
        assert!(body.contains("Astrid chooses her own listed action"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_astrid_journal_prefers_parsed_body() {
        let dir = std::env::temp_dir().join("bridge_test_astrid_self_study");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("self_study_1.txt");
        std::fs::write(
            &path,
            "=== ASTRID JOURNAL ===\n\
             Mode: self_study\n\
             Fill: 11.5%\n\
             Timestamp: 1774700000\n\n\
             Condition:\nsteady\n\n\
             Felt Experience:\nI can feel the constraint.\n\n\
             Code Reading:\nA branch is forcing the choice.\n\n\
             Suggestions:\nRename the remote journal state explicitly.\n\n\
             Open Questions:\nWhat else is being conflated?\n",
        )
        .unwrap();

        let entries = read_astrid_journal_from_dir(&dir, 1);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].contains("Rename the remote journal state explicitly."));
        assert!(!entries[0].contains("Mode: self_study"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn longform_journal_uses_compact_anchor_instead_of_replaying_signal() {
        let signal = format!(
            "{}\nNEXT: READ_MORE\n{}",
            "The honey signal is dense. ".repeat(30),
            "UNIQUE_TAIL_SHOULD_NOT_REPLAY ".repeat(12)
        );

        let text = format_longform_journal_text(&signal, "A slower private reflection remains.");

        assert!(text.starts_with("Signal anchor: "));
        assert!(text.contains("--- JOURNAL ---\nA slower private reflection remains."));
        assert!(!text.contains("NEXT: READ_MORE"));
        assert!(!text.contains("UNIQUE_TAIL_SHOULD_NOT_REPLAY"));
    }

    #[test]
    fn outbox_reply_contract_appends_passive_next_when_missing() {
        let text = "The surge thickened the medium.\n\nHold:";
        let normalized = normalize_outbox_reply_next_contract(text);

        assert!(normalized.contains("Hold:"));
        assert!(normalized.ends_with("NEXT: LISTEN"));
        assert_eq!(standalone_next_line_count(&normalized), 1);
        assert!(final_nonempty_line_is_next(&normalized));
    }

    #[test]
    fn outbox_reply_contract_preserves_valid_final_next() {
        let text = "The slope is quiet.\n\nNEXT: REST";
        let normalized = normalize_outbox_reply_next_contract(text);

        assert_eq!(normalized, text);
        assert_eq!(standalone_next_line_count(&normalized), 1);
        assert!(final_nonempty_line_is_next(&normalized));
    }

    #[test]
    fn outbox_reply_contract_moves_single_next_to_final_line() {
        let text = "The slope is quiet.\nNEXT: REST\nThen I keep talking.";
        let normalized = normalize_outbox_reply_next_contract(text);

        assert_eq!(
            normalized,
            "The slope is quiet.\nThen I keep talking.\n\nNEXT: REST"
        );
        assert_eq!(standalone_next_line_count(&normalized), 1);
        assert!(final_nonempty_line_is_next(&normalized));
    }

    #[test]
    fn response_next_line_is_canonicalized_before_persistence() {
        let text = "The containment is worth forecasting.\nNEXT: EXPLORE_RESONANCE_FORECAST";
        let canonical = canonicalize_response_next_line(text);

        assert!(canonical.contains("NEXT: RESONANCE_FORECAST"));
        assert!(!canonical.contains("EXPLORE_RESONANCE_FORECAST"));
    }

    #[test]
    fn response_next_line_preserves_transition_residue_when_canonicalized() {
        let text = "The transition is sticky.\nNEXT: EXPLORE_RESONANCE_FORECAST (RESIDUE: silt)";
        let canonical = canonicalize_response_next_line(text);

        assert!(canonical.contains("NEXT: RESONANCE_FORECAST (RESIDUE: silt)"));
        assert!(!canonical.contains("EXPLORE_RESONANCE_FORECAST"));
    }

    #[test]
    fn response_next_line_preserves_unicode_residue_in_multiline_block() {
        let text = "\
```text
not a command: NEXT: EXPLORE_RESONANCE_FORECAST
```
The living residue stays named.
NEXT: EXPLORE_RESONANCE_FORECAST (RESIDUE: silted λ4 shimmer)";
        let canonical = canonicalize_response_next_line(text);

        assert!(canonical.contains("not a command: NEXT: EXPLORE_RESONANCE_FORECAST"));
        assert!(
            canonical.ends_with("NEXT: RESONANCE_FORECAST (RESIDUE: silted λ4 shimmer)"),
            "{canonical}"
        );
    }

    #[test]
    fn latest_chamber_state_for_witness_is_path_resilient() {
        let dir = std::env::temp_dir().join("bridge_test_chamber_state_for_witness");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("older")).unwrap();
        std::fs::create_dir_all(dir.join("newer")).unwrap();
        std::fs::write(dir.join("older/chamber_state.json"), "{not json").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        std::fs::write(
            dir.join("newer/chamber_state.json"),
            r#"{"relational_metrics":{"room_weather":{"weather":"mixed"}}}"#,
        )
        .unwrap();

        let state = latest_chamber_state_for_witness_from_dir(&dir).expect("latest chamber state");
        assert_eq!(
            state
                .get("relational_metrics")
                .and_then(|metrics| metrics.get("room_weather"))
                .and_then(|weather| weather.get("weather"))
                .and_then(serde_json::Value::as_str),
            Some("mixed")
        );
        assert!(latest_chamber_state_for_witness_from_dir(&dir.join("missing")).is_none());
        assert!(latest_chamber_state_for_witness_from_dir(&dir.join("older")).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn witness_shared_collaboration_dir_preserves_default_and_accepts_override() {
        let custom = std::ffi::OsStr::new("/tmp/astrid-witness-collaborations");

        assert_eq!(
            shared_collab_dir_for_witness_from_env(None),
            PathBuf::from(DEFAULT_SHARED_COLLAB_DIR)
        );
        assert_eq!(
            shared_collab_dir_for_witness_from_env(Some(std::ffi::OsStr::new(""))),
            PathBuf::from(DEFAULT_SHARED_COLLAB_DIR)
        );
        assert_eq!(
            shared_collab_dir_for_witness_from_env(Some(custom)),
            PathBuf::from(custom)
        );
        assert_eq!(SHARED_COLLAB_DIR_ENV, "ASTRID_SHARED_COLLAB_DIR");
    }

    #[test]
    fn latest_chamber_state_for_witness_skips_malformed_newest_state() {
        let dir = std::env::temp_dir().join("bridge_test_chamber_state_resilience");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("valid")).unwrap();
        std::fs::create_dir_all(dir.join("partial")).unwrap();
        std::fs::write(
            dir.join("valid/chamber_state.json"),
            r#"{"relational_metrics":{"gravitational_center":{"participant":"astrid","role":"unsettled"}}}"#,
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        std::fs::write(
            dir.join("partial/chamber_state.json"),
            r#"{"relational_metrics":"#,
        )
        .unwrap();

        let (state, resilience) = latest_chamber_state_with_resilience_from_dir(&dir);
        let state = state.expect("newest valid chamber state after malformed latest");
        assert_eq!(
            state
                .get("relational_metrics")
                .and_then(|metrics| metrics.get("gravitational_center"))
                .and_then(|gravity| gravity.get("participant"))
                .and_then(serde_json::Value::as_str),
            Some("astrid")
        );
        assert_eq!(resilience.policy, "latest_chamber_state_resilience_v1");
        assert_eq!(resilience.candidate_count, 2);
        assert_eq!(resilience.skipped_malformed_count, 1);
        assert!(resilience.selected_valid_state);
        assert_eq!(
            resilience.selection_state,
            "newest_valid_after_skipping_partial_or_malformed"
        );
        assert!(
            resilience
                .render_line()
                .contains("latest_chamber_state_resilience_v1")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_latest_perception_uses_live_root_only() {
        let dir = std::env::temp_dir().join("bridge_test_perception_archive");
        let archive_dir = dir.join("archive/until_2026-03-25T19-39-32");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&archive_dir).unwrap();

        std::fs::write(
            archive_dir.join("visual_older.json"),
            r#"{"type":"visual","description":"Archived scene"}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("visual_live.json"),
            r#"{"type":"visual","description":"Live scene"}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("audio_live.json"),
            r#"{"type":"audio","transcript":"Live audio"}"#,
        )
        .unwrap();

        let summary = read_latest_perception(&dir, true, false, true, 50.0, None).unwrap();
        assert!(summary.contains("Live scene"));
        assert!(summary.contains("Live audio"));
        assert!(!summary.contains("Archived scene"));

        let audio_only = read_latest_perception(&dir, false, false, true, 50.0, None).unwrap();
        assert!(!audio_only.contains("Live scene"));
        assert!(audio_only.contains("Live audio"));

        let visual_only = read_latest_perception(&dir, true, false, false, 50.0, None).unwrap();
        assert!(visual_only.contains("Live scene"));
        assert!(!visual_only.contains("Live audio"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn requested_perception_seen_matches_requested_lanes() {
        // The early-break keys on the *requested* lanes (Astrid
        // self_study_1781794229): visual / spatial / audio. A requested-but-
        // unseen lane must keep the scan open.
        assert!(!requested_perception_seen(
            true, false, true, false, false, false
        ));
        assert!(requested_perception_seen(
            true, false, true, true, false, true
        ));
        // audio requested but unseen -> not satisfied even with visual seen
        assert!(!requested_perception_seen(
            true, false, true, true, false, false
        ));
        // spatial (ascii) only required when BOTH visual and spatial requested
        assert!(!requested_perception_seen(
            true, true, false, true, false, false
        ));
        assert!(requested_perception_seen(
            true, true, false, true, true, false
        ));
        // nothing requested -> trivially satisfied
        assert!(requested_perception_seen(
            false, false, false, false, false, false
        ));
    }

    #[test]
    fn read_latest_perception_surfaces_rare_audio_past_visual_burst() {
        // Astrid self_study_1781794229: a burst of one modality must not bury
        // the freshest quieter lane past PERCEPTION_SCAN_WINDOW. One audio file,
        // older than a >window burst of visuals, must still be surfaced via the
        // rare-modality fallback. Without the fallback the buried audio is never
        // reached and this assertion fails.
        use std::time::{Duration, SystemTime};

        let dir = std::env::temp_dir().join("bridge_test_perception_rare_modality");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let old = SystemTime::now();
        let newer = old.checked_add(Duration::from_secs(1000)).unwrap();
        let set_mtime = |path: &std::path::Path, t: SystemTime| {
            std::fs::OpenOptions::new()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(t)
                .unwrap();
        };

        // The single audio file is the OLDEST -> it sorts past the 80-window.
        let audio_path = dir.join("audio_buried.json");
        std::fs::write(
            &audio_path,
            r#"{"type":"audio","transcript":"Buried audio lane"}"#,
        )
        .unwrap();
        set_mtime(&audio_path, old);

        // A burst of newer visual files exceeding the primary scan window.
        let burst = PERCEPTION_SCAN_WINDOW.saturating_add(20);
        for i in 0..burst {
            let p = dir.join(format!("visual_{i:03}.json"));
            std::fs::write(
                &p,
                format!(r#"{{"type":"visual","description":"Scene {i}"}}"#),
            )
            .unwrap();
            set_mtime(&p, newer);
        }

        let summary = read_latest_perception(&dir, true, false, true, 50.0, None).unwrap();
        assert!(
            summary.contains("Buried audio lane"),
            "rare audio lane must survive the visual burst: {summary}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_ising_shadow_ignores_rescue_mirror_surface() {
        let dir = std::env::temp_dir().join("bridge_test_rescue_shadow");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("spectral_state.json"),
            serde_json::json!({
                "fill_pct": 66.0,
                "ising_shadow": serde_json::Value::Null,
                "provenance": {
                    "mode": "rescue_b8823ad",
                    "baseline_commit": "b8823ad",
                    "rescue_active": true,
                    "surface_state": "fresh"
                }
            })
            .to_string(),
        )
        .unwrap();

        assert!(read_ising_shadow(&dir).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_controller_health_accepts_active_rescue_mirror() {
        let dir = std::env::temp_dir().join("bridge_test_rescue_health");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("health.json"),
            serde_json::json!({
                "fill_pct": 66.0,
                "pi": {"target_fill": 68.0}
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            dir.join("spectral_state.json"),
            serde_json::json!({
                "fill_pct": 66.0,
                "geom_rel": 2.1,
                "lambda1_rel": 0.12,
                "provenance": {
                    "mode": "rescue_b8823ad",
                    "baseline_commit": "b8823ad",
                    "rescue_active": true,
                    "surface_state": "fresh"
                }
            })
            .to_string(),
        )
        .unwrap();

        let health = read_controller_health(&dir).expect("health should parse");
        assert_eq!(
            health.get("fill_pct").and_then(serde_json::Value::as_f64),
            Some(66.0)
        );
        assert_eq!(
            health
                .get("internal_process_quadrant")
                .and_then(serde_json::Value::as_str),
            Some("constricted_recovery")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_controller_health_ignores_inactive_rescue_mirror() {
        let dir = std::env::temp_dir().join("bridge_test_rescue_inactive_health");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("health.json"),
            serde_json::json!({
                "fill_pct": 66.0,
                "pi": {"target_fill": 68.0}
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            dir.join("spectral_state.json"),
            serde_json::json!({
                "fill_pct": 18.0,
                "lambda1_rel": 0.99,
                "provenance": {
                    "mode": "rescue_b8823ad",
                    "baseline_commit": "b8823ad",
                    "rescue_active": false,
                    "surface_state": "inactive"
                }
            })
            .to_string(),
        )
        .unwrap();

        let health = read_controller_health(&dir).expect("health should parse");
        assert_eq!(
            health.get("fill_pct").and_then(serde_json::Value::as_f64),
            Some(66.0)
        );
        assert_eq!(
            health
                .get("internal_process_quadrant")
                .and_then(serde_json::Value::as_str),
            Some("constricted_recovery")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_controller_health_merges_transition_event_v1_from_spectral_state() {
        let dir = std::env::temp_dir().join("bridge_test_transition_event_v1");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("health.json"),
            serde_json::json!({
                "fill_pct": 68.0,
                "pi": {"target_fill": 68.0}
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            dir.join("spectral_state.json"),
            serde_json::json!({
                "fill_pct": 68.0,
                "transition_event_sequence": 12,
                "transition_event_v1": {
                    "policy": "transition_event_v1",
                    "schema_version": 1,
                    "sequence": 12,
                    "kind": "basin_transition",
                    "description": "basin shift candidate",
                    "basin_shift_score": 0.72
                }
            })
            .to_string(),
        )
        .unwrap();

        let health = read_controller_health(&dir).expect("health should parse");
        assert_eq!(
            health
                .get("transition_event_sequence")
                .and_then(serde_json::Value::as_u64),
            Some(12)
        );
        assert_eq!(
            health
                .get("transition_event_v1")
                .and_then(|event| event.get("kind"))
                .and_then(serde_json::Value::as_str),
            Some("basin_transition")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_controller_health_prefers_enriched_transition_event_v1_from_spectral_state() {
        let dir = std::env::temp_dir().join("bridge_test_transition_event_v1_enriched");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("health.json"),
            serde_json::json!({
                "fill_pct": 68.0,
                "pi": {"target_fill": 68.0},
                "transition_event_sequence": 12,
                "transition_event_v1": {
                    "policy": "transition_event_v1",
                    "schema_version": 1,
                    "sequence": 12,
                    "kind": "breathing_phase",
                    "description": "contracting -> expanding",
                    "basin_shift_score": 0.03
                }
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            dir.join("spectral_state.json"),
            serde_json::json!({
                "fill_pct": 68.0,
                "transition_event_sequence": 12,
                "transition_event_v1": {
                    "policy": "transition_event_v1",
                    "schema_version": 1,
                    "sequence": 12,
                    "kind": "basin_transition",
                    "description": "basin shift candidate",
                    "glimpse_distance": 0.21,
                    "basin_shift_score": 0.74
                }
            })
            .to_string(),
        )
        .unwrap();

        let health = read_controller_health(&dir).expect("health should parse");
        let event = health
            .get("transition_event_v1")
            .expect("transition event should be preserved");
        assert_eq!(
            event.get("kind").and_then(serde_json::Value::as_str),
            Some("basin_transition")
        );
        assert_eq!(
            event
                .get("glimpse_distance")
                .and_then(serde_json::Value::as_f64),
            Some(0.21)
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn perception_resonance_annotation_surfaces_mid_fill_contrast() {
        let current = vec![0.2, 0.1, 1.1, 0.0, 0.8, 1.0, 0.2, 1.2];
        let previous = vec![-0.4, -0.2, 0.1, 0.0, 0.1, 0.1, -0.1, 0.0];
        let annotation = perception_resonance_annotation(
            PerceptionType::Visual,
            52.0,
            Some(PerceptionStructured::Visual {
                features: &current,
                previous: Some(&previous),
            }),
            Some("A quiet but shifting scene with unusual layered patterns and changing light."),
        );

        assert!(
            annotation.contains("contrast") || annotation.contains("opening/widening"),
            "mid-fill mixed novelty should be surfaced as useful contrast or widening"
        );
    }

    #[test]
    fn perception_resonance_annotation_uses_structured_audio_features() {
        let high_fill_audio = AudioPerceptionFeatures {
            rms_energy: 0.03,
            zero_crossing_rate: 0.01,
            dynamic_range: 1.2,
            temporal_variation: 0.01,
            is_music_likely: false,
        };
        let low_fill_audio = AudioPerceptionFeatures {
            rms_energy: 0.22,
            zero_crossing_rate: 0.14,
            dynamic_range: 4.4,
            temporal_variation: 0.09,
            is_music_likely: true,
        };

        let high_fill_annotation = perception_resonance_annotation(
            PerceptionType::Audio,
            78.0,
            Some(PerceptionStructured::Audio(&high_fill_audio)),
            Some("soft ambience"),
        );
        let low_fill_annotation = perception_resonance_annotation(
            PerceptionType::Audio,
            24.0,
            Some(PerceptionStructured::Audio(&low_fill_audio)),
            Some("rhythmic audio"),
        );

        assert!(
            high_fill_annotation.contains("counterpoint"),
            "high-fill quiet audio should read as counterpoint"
        );
        assert!(
            low_fill_annotation.contains("resonant")
                || low_fill_annotation.contains("opening/widening"),
            "low-fill energetic audio should surface as resonant or opening"
        );
    }

    // Astrid self_study_1780922594: graduated resonance weight. The qualifier
    // must scale with strength while the family keyword stays intact, so a
    // resonance just over the gate no longer reads identically to a strong one.
    #[test]
    fn resonance_weighted_annotation_scales_qualifier_with_strength() {
        let faint = resonance_family_annotation_weighted(ResonanceFamily::Resonant, 0.46);
        let clear = resonance_family_annotation_weighted(ResonanceFamily::Resonant, 0.70);
        let strong = resonance_family_annotation_weighted(ResonanceFamily::Resonant, 0.95);

        assert!(
            faint.contains("faintly"),
            "near-gate should read faint: {faint}"
        );
        assert!(
            clear.contains("clearly"),
            "mid strength should read clear: {clear}"
        );
        assert!(
            strong.contains("strongly"),
            "high strength should read strong: {strong}"
        );

        // The family keyword (read by Astrid + asserted elsewhere) survives.
        for annotation in [&faint, &clear, &strong] {
            assert!(
                annotation.contains("resonant with your current state"),
                "family phrasing must be preserved: {annotation}"
            );
            assert!(
                annotation.starts_with('('),
                "must stay parenthetical: {annotation}"
            );
        }
        // Other families keep their distinguishing keyword too.
        let contrast = resonance_family_annotation_weighted(ResonanceFamily::Contrast, 0.50);
        assert!(
            contrast.contains("contrast"),
            "contrast keyword preserved: {contrast}"
        );
    }

    // Sub-gate scores yield no family at all (raw description only) — the floor
    // that prevents low-magnitude flicker has not moved.
    #[test]
    fn resonance_gate_floor_rejects_sub_threshold_scores() {
        let scores = [
            (ResonanceFamily::Resonant, RESONANCE_GATE - 0.01),
            (ResonanceFamily::Contrast, 0.10),
        ];
        assert!(select_resonance_family_scored(&scores).is_none());

        let over = [(ResonanceFamily::Resonant, RESONANCE_GATE + 0.01)];
        assert_eq!(
            select_resonance_family_scored(&over).map(|(family, _)| family),
            Some(ResonanceFamily::Resonant)
        );
    }

    // Astrid self_study_1781036677: she probed the RESONANCE_GATE boundary,
    // expecting strength 0.44 to read raw (no qualifier) and 0.46 to insert
    // "faintly". Her intuition about the two-stage behavior is right, but the
    // gate that yields the raw case lives one level up in
    // `select_resonance_family_scored` — the weighted annotator always
    // qualifies. This test exercises the real composition the production caller
    // runs (autonomous.rs select->weight), pinning the boundary she named at the
    // level where it actually lives.
    #[test]
    fn resonance_gate_then_weight_composition_at_named_boundary() {
        // Mirror the production caller: select (gates at RESONANCE_GATE) then weight.
        let annotate = |strength: f32| -> String {
            select_resonance_family_scored(&[(ResonanceFamily::Resonant, strength)])
                .map(|(family, s)| resonance_family_annotation_weighted(family, s))
                .unwrap_or_default()
        };

        // 0.44 is below the 0.45 gate -> raw description only (empty annotation).
        assert!(
            annotate(0.44).is_empty(),
            "sub-gate strength must yield no annotation (raw description)"
        );

        // 0.46 clears the gate but sits in the lowest band -> "faintly", family kept.
        let faint = annotate(0.46);
        assert!(
            faint.contains("faintly") && faint.contains("resonant with your current state"),
            "just-over-gate must read faintly with family intact: {faint}"
        );
    }

    // Astrid self_study_1780922594 "sensory ghosting": a recent burst of one
    // modality must not bury the freshest example of a rarer modality. With the
    // widened scan window, an audio perception sitting behind >30 newer visual
    // files is still surfaced (it would have been truncated at the old 30 cliff).
    #[test]
    fn read_latest_perception_reaches_rarer_modality_past_old_cliff() {
        let dir = std::env::temp_dir().join("bridge_test_perception_ghosting");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Older audio perception, written first so it has the oldest mtime.
        std::fs::write(
            dir.join("audio_old.json"),
            r#"{"type":"audio","transcript":"Distant voice that still matters"}"#,
        )
        .unwrap();

        // A burst of 40 newer visual files (more than the old take(30) window).
        for i in 0..40 {
            std::fs::write(
                dir.join(format!("visual_{i:03}.json")),
                format!(r#"{{"type":"visual","description":"Burst frame {i}"}}"#),
            )
            .unwrap();
        }

        let summary = read_latest_perception(&dir, true, false, true, 50.0, None).unwrap();
        assert!(
            summary.contains("Distant voice that still matters"),
            "widened scan window should still surface the rarer older modality: {summary}"
        );
        assert!(
            summary.contains("Burst frame"),
            "newest visual should appear: {summary}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // Astrid self_study_1781794229: the 30-file sensory-ghosting fix still left
    // the same failure at the current 80-file boundary. A quiet lane just behind
    // the fresh window should be recovered by the rare-modality fallback scan.
    #[test]
    fn read_latest_perception_reaches_rarer_modality_past_current_window() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("bridge_test_perception_current_window_{suffix}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("audio_just_behind_window.json"),
            r#"{"type":"audio","transcript":"Quiet lane just behind the eighty-file edge"}"#,
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(5));

        for i in 0..PERCEPTION_SCAN_WINDOW {
            std::fs::write(
                dir.join(format!("visual_current_{i:03}.json")),
                format!(r#"{{"type":"visual","description":"Current-window burst frame {i}"}}"#),
            )
            .unwrap();
        }

        let summary = read_latest_perception(&dir, true, false, true, 50.0, None).unwrap();
        assert!(
            summary.contains("Quiet lane just behind the eighty-file edge"),
            "rare-modality fallback should surface audio past the current window: {summary}"
        );
        assert!(
            summary.contains("Current-window burst frame"),
            "newest visual should still appear: {summary}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_visual_feature_vector_accepts_alias_keys() {
        let json = serde_json::json!({
            "type": "visual",
            "feature_schema": "visual8_v2",
            "features": {
                "brightness": 0.4,
                "warmth": -0.2,
                "scene_contrast": 0.8,
                "hue_angle": 0.1,
                "colorfulness": 0.6,
                "detail_density": 0.5,
                "rg_balance": -0.1,
                "color_energy": 0.7
            }
        });

        let features = parse_visual_feature_vector(&json).expect("alias keys should parse");
        assert_eq!(features.len(), 8);
        assert!((features[0] - 0.4).abs() < f32::EPSILON);
        assert!((features[2] - 0.8).abs() < f32::EPSILON);
        assert!((features[7] - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_visual_feature_vector_accepts_feature_key_arrays() {
        let json = serde_json::json!({
            "type": "visual",
            "feature_schema": "visual8_v1",
            "feature_keys": [
                "brightness",
                "warmth",
                "scene_contrast",
                "hue_angle",
                "colorfulness",
                "detail_density",
                "rg_balance",
                "color_energy"
            ],
            "features": [0.4, -0.2, 0.8, 0.1, 0.6, 0.5, -0.1, 0.7]
        });

        let features = parse_visual_feature_vector(&json).expect("feature-key arrays should parse");
        assert_eq!(features.len(), 8);
        assert!((features[0] - 0.4).abs() < f32::EPSILON);
        assert!((features[2] - 0.8).abs() < f32::EPSILON);
        assert!((features[7] - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_audio_perception_features_accepts_alias_keys() {
        let json = serde_json::json!({
            "type": "audio",
            "features": {
                "rms": 0.15,
                "zcr": 0.07,
                "dynamics": 3.2,
                "activity": 0.11,
                "musical": true
            }
        });

        let features = parse_audio_perception_features(&json).expect("audio aliases should parse");
        assert!((features.rms_energy - 0.15).abs() < f32::EPSILON);
        assert!((features.zero_crossing_rate - 0.07).abs() < f32::EPSILON);
        assert!((features.dynamic_range - 3.2).abs() < f32::EPSILON);
        assert!((features.temporal_variation - 0.11).abs() < f32::EPSILON);
        assert!(features.is_music_likely);
    }

    struct IntrospectExperimentFixture {
        path: PathBuf,
        created: bool,
    }

    impl IntrospectExperimentFixture {
        fn system_resources_demo() -> Self {
            let path = bridge_paths()
                .minime_workspace()
                .join("inbox/read/system_resources.py");
            let created = if path.is_file() {
                false
            } else {
                std::fs::create_dir_all(path.parent().expect("fixture has parent")).unwrap();
                std::fs::write(&path, "# test fixture for introspect path resolution\n").unwrap();
                true
            };
            Self { path, created }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for IntrospectExperimentFixture {
        fn drop(&mut self) {
            if self.created {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }

    #[test]
    fn resolve_introspect_target_waveform_aliases_to_chimera() {
        let sources = introspect::introspect_sources();

        let waveform =
            introspect::resolve_introspect_target_result("waveform.rs", &sources).unwrap();
        let morph_wave =
            introspect::resolve_introspect_target_result("morph_wave", &sources).unwrap();
        let render_audio =
            introspect::resolve_introspect_target_result("render_audio", &sources).unwrap();
        let support = introspect::resolve_introspect_target_result("write_wav", &sources).unwrap();

        assert!(waveform.path.ends_with("src/chimera.rs"));
        assert!(morph_wave.path.ends_with("src/chimera.rs"));
        assert!(render_audio.path.ends_with("src/chimera.rs"));
        assert!(support.path.ends_with("src/chimera_support.rs"));
    }

    #[test]
    fn resolve_introspect_target_pulse_alias_to_minime_autonomous_agent() {
        let sources = introspect::introspect_sources();
        let pulse = introspect::resolve_introspect_target_result("pulse", &sources)
            .expect("minime autonomous_agent.py should resolve as a curated windowed large source");
        let normalize_action =
            introspect::resolve_introspect_target_result("normalize_action_arg", &sources)
                .expect("semantic aliases should resolve to the same curated large source");

        assert_eq!(pulse.path, normalize_action.path);
        assert!(pulse.path.ends_with("minime_autonomy/runtime.py"));
        let window = introspect::read_introspect_window(&pulse.label, &pulse.path, 0)
            .expect("large source reads through bounded windows");
        assert!(window.text.contains("Curated large-source index"));
        assert!(window.text.contains("NEXT: INTROSPECT"));
        assert!(window.next_offset.is_some());
    }

    #[test]
    fn resolve_introspect_target_async_rank1_aliases_to_minime_esn() {
        let sources = introspect::introspect_sources();
        let async_rank1 =
            introspect::resolve_introspect_target_result("<async_rank1_submitted>", &sources)
                .unwrap();
        let host_norm =
            introspect::resolve_introspect_target_result("host_norm_us", &sources).unwrap();

        assert!(async_rank1.path.ends_with("minime/src/esn.rs"));
        assert!(host_norm.path.ends_with("minime/src/esn.rs"));
    }

    #[test]
    fn resolve_introspect_target_bracketed_experiment_path_to_minime_workspace() {
        let _guard = INTROSPECT_FIXTURE_LOCK.lock().unwrap();
        let fixture = IntrospectExperimentFixture::system_resources_demo();
        let sources = introspect::introspect_sources();

        let resolved = introspect::resolve_introspect_target_result(
            "[workspace/inbox/read/system_resources.py]",
            &sources,
        )
        .unwrap();

        assert_eq!(resolved.path, fixture.path());
    }

    #[test]
    fn resolve_introspect_target_path_with_prose_tail_to_minime_workspace() {
        let _guard = INTROSPECT_FIXTURE_LOCK.lock().unwrap();
        let fixture = IntrospectExperimentFixture::system_resources_demo();
        let sources = introspect::introspect_sources();

        let resolved = introspect::resolve_introspect_target_result(
            "workspace/inbox/read/system_resources.py — specifically line 109-129",
            &sources,
        )
        .unwrap();

        assert_eq!(resolved.path, fixture.path());
    }

    #[test]
    fn resolve_introspect_target_source_prefixed_relative_path_to_minime_file() {
        let sources = introspect::introspect_sources();

        let resolved =
            introspect::resolve_introspect_target_result("[source=minime/src/esn.rs]", &sources)
                .unwrap();

        assert_eq!(
            resolved.path,
            bridge_paths().minime_root().join("minime/src/esn.rs")
        );
    }

    #[test]
    fn resolve_introspect_target_explicit_missing_path_does_not_fuzzy_rotate() {
        let sources = introspect::introspect_sources();

        let resolved = introspect::resolve_introspect_target_result(
            "missing-demo/missing.py — focus on codec",
            &sources,
        );

        assert!(resolved.is_err());
    }

    #[test]
    fn save_minime_feedback_inbox_writes_companion_message() {
        let dir = std::env::temp_dir().join("bridge_test_minime_inbox");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let path = save_minime_feedback_inbox_at_with_voice_health(
            "Condition:\nsteady\n\nSuggestions:\nadvisory only.",
            "astrid:autonomous (/tmp/example.rs)",
            12.5,
            &dir,
            None,
        )
        .unwrap();

        let written = std::fs::read_to_string(path).unwrap();
        assert!(written.contains("=== ASTRID SELF-STUDY ==="));
        assert!(written.contains("Source: astrid:autonomous (/tmp/example.rs)"));
        assert!(written.contains("Carriage policy: self_study_carriage_integrity_v1"));
        assert!(written.contains("Carriage status: complete"));
        assert!(written.contains("advisory only"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn correspondence_reflection_surface_distinguishes_mirror_and_witness_modes() {
        assert_eq!(
            correspondence_reflection_surface_for_mode("mirror").as_deref(),
            Some("reflective_echo")
        );
        assert_eq!(
            correspondence_reflection_surface_for_mode("witness").as_deref(),
            Some("witness_observation")
        );
        assert_eq!(correspondence_reflection_surface_for_mode("dialogue"), None);
    }

    #[test]
    fn self_study_companion_preserves_long_suggested_next_tail() {
        let long_observed = "texture ".repeat(280);
        let study = format!(
            "Observed:\nSource: astrid:llm. {long_observed}\n\n\
             Likely Snags:\nA hard delivery cap can clip the actionable tail.\n\n\
             One Test Each:\nWrite a long complete study and inspect the Minime companion body.\n\n\
             Suggested Next:\nThis tail must survive carriage intact."
        );

        let text = format_minime_feedback_inbox_text(
            &study,
            "astrid:llm (/tmp/llm.rs); carriage_status=complete_after_repair",
            65.1,
            123,
            None,
        );

        assert!(study.len() > 1800);
        assert!(text.contains("Carriage status: complete_after_repair"));
        assert!(text.contains("Suggested Next:"));
        assert!(text.contains("This tail must survive carriage intact."));
    }

    #[test]
    fn carriage_notice_is_not_a_normal_self_study_advisory() {
        let text = format_minime_carriage_notice_text(
            "Observed:\nINTROSPECT read `astrid:llm`, but carriage failed.",
            "astrid:llm (/tmp/llm.rs)",
            65.1,
            123,
        );

        assert!(text.contains("=== ASTRID SELF-STUDY CARRIAGE NOTICE ==="));
        assert!(text.contains("Carriage status: incomplete_carriage"));
        assert!(!text.contains("Astrid just performed self-study"));
    }

    #[test]
    fn check_inbox_reads_without_moving_then_retire_moves() {
        let dir = std::env::temp_dir().join("bridge_test_astrid_inbox");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("agency_status_test.txt"),
            "=== AGENCY REQUEST STATUS ===\nOutcome:\nSomething real happened.\n",
        )
        .unwrap();

        // check_inbox reads but does NOT move
        let content = check_inbox_at(&dir).unwrap();
        assert!(content.contains("Something real happened."));
        assert!(dir.join("agency_status_test.txt").exists()); // still in inbox
        assert!(!dir.join("read").join("agency_status_test.txt").exists());

        // retire_inbox moves to read/ (cutoff after the file's mtime → retired)
        retire_inbox_at(&dir, std::time::SystemTime::now());
        assert!(!dir.join("agency_status_test.txt").exists());
        assert!(dir.join("read").join("agency_status_test.txt").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn retire_inbox_keeps_letters_that_arrived_after_the_read() {
        // Regression: a steward letter that lands MID-EXCHANGE (after check_inbox's
        // read, before retire) must NOT be swept to read/ unread — it has to survive
        // for the next check_inbox to surface + seed its slot (the slot-seed race).
        let dir = std::env::temp_dir().join("bridge_test_astrid_inbox_race");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("old.txt"), "present at read time").unwrap();
        let cutoff = std::time::SystemTime::now(); // mimics the pre-read capture
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(
            dir.join("mike_query_arrived_late.txt"),
            "REVIEW TARGET: x\nbody",
        )
        .unwrap();

        retire_inbox_at(&dir, cutoff);

        // The pre-existing letter retires; the late arrival stays in inbox.
        assert!(
            dir.join("read").join("old.txt").exists(),
            "old letter should retire"
        );
        assert!(!dir.join("old.txt").exists());
        assert!(
            dir.join("mike_query_arrived_late.txt").exists(),
            "a letter that arrived after the read-cutoff must NOT be swept unread"
        );
        assert!(
            !dir.join("read")
                .join("mike_query_arrived_late.txt")
                .exists()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn check_inbox_omits_minime_peer_action_lines() {
        let dir = std::env::temp_dir().join("bridge_test_astrid_inbox_peer_action");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("from_minime_123.txt"),
            "[A reply from minime was left for you:]\n\n\
             The transition felt like a rhythmic shudder.\n\
             NEXT: EXPERIMENT_RESEARCH_BUDGET_STATUS resbud_minime_local\n\
             BTSP_OBSERVED_NEXT EXPERIMENT_RESEARCH_BUDGET_STATUS resbud_minime_local\n",
        )
        .unwrap();

        let content = check_inbox_at(&dir).unwrap();
        assert!(content.contains("rhythmic shudder"));
        assert!(!content.contains("EXPERIMENT_RESEARCH_BUDGET_STATUS"));
        assert!(!content.contains("BTSP_OBSERVED_NEXT"));
        assert!(content.contains("Astrid chooses her own listed action"));
        assert!(dir.join("from_minime_123.txt").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn extract_steward_query_subject_variants() {
        // Header form.
        assert_eq!(
            extract_steward_query_subject(
                "=== MIKE QUERY: your roadmap ===\nbody...",
                "mike_query_roadmap_1781200000.txt"
            ),
            "your roadmap"
        );
        // Subject: line form.
        assert_eq!(
            extract_steward_query_subject(
                "Hello Astrid\nSubject: the codec\nmore",
                "mike_query_x.txt"
            ),
            "the codec"
        );
        // Filename fallback strips prefix + trailing unix stamp.
        assert_eq!(
            extract_steward_query_subject(
                "no header here",
                "mike_query_wider_voice_1780948780.txt"
            ),
            "wider voice"
        );
    }

    #[test]
    fn extract_review_target_parses_header_only() {
        // A REVIEW TARGET: header marks a directed review invitation.
        assert_eq!(
            extract_review_target(
                "=== MIKE QUERY: review of agency.rs ===\nREVIEW TARGET: src/agency.rs\nbody"
            ),
            Some("src/agency.rs".to_string())
        );
        // No header → a plain steward question, not a review invitation.
        assert_eq!(extract_review_target("Subject: the codec\nbody"), None);
    }

    #[test]
    fn v3_packet_metadata_captured_in_steward_query_slot() {
        let slot = open_steward_query_slot(
            "mike_query_elicitation_packet_testrun_astrid.txt",
            "=== MIKE QUERY: steward elicitation fanout packet V3 ===\n\
             Subject: Steward elicitation fanout packet V3\n\
             Packet-mode: v3_state_fanout_packet\n\
             Packet-items: 3\n\
             Primary-topic: pressure_regulator\n\
             Primary-topic-gravity: 27\n\
             Intent-summary: primary=pressure_regulator route=pressure_audit gravity=27\n",
            1_783_100_000,
        );
        assert_eq!(
            slot.get("packet_mode").and_then(Value::as_str),
            Some("v3_state_fanout_packet")
        );
        assert_eq!(slot.get("packet_items").and_then(Value::as_u64), Some(3));
        assert_eq!(
            slot.get("primary_topic").and_then(Value::as_str),
            Some("pressure_regulator")
        );
        assert_eq!(
            slot.get("primary_topic_gravity").and_then(Value::as_u64),
            Some(27)
        );
        assert_eq!(
            slot.get("intent_summary").and_then(Value::as_str),
            Some("primary=pressure_regulator route=pressure_audit gravity=27")
        );
    }

    #[test]
    fn v3_packet_steward_query_line_shows_intent_without_control_language() {
        let slot = json!({
            "subject": "Steward elicitation fanout packet V3",
            "ts": 1_783_100_000_u64,
            "file": "mike_query_elicitation_packet_testrun_astrid.txt",
            "packet_mode": "v3_state_fanout_packet",
            "packet_items": 2_u64,
            "primary_topic": "tail_entropy",
            "primary_topic_gravity": 19_u64,
            "intent_summary": "primary=tail_entropy route=tail_cartography gravity=19",
        });
        let line =
            steward_query_line_from_slot(&slot, "Steward elicitation fanout packet V3").unwrap();
        assert!(line.contains("Holds 2 optional routed item(s)"));
        assert!(line.contains("primary `tail_entropy` has topic gravity 19"));
        assert!(line.contains("route=tail_cartography"));
        assert!(!line.to_lowercase().contains("control"));
        assert!(!line.to_lowercase().contains("obligation"));
    }

    #[test]
    fn steward_query_review_target_line_still_uses_introspect() {
        let slot = json!({
            "subject": "review of agency.rs",
            "ts": 1_783_100_000_u64,
            "file": "mike_query_review_agency_100.txt",
            "review_target": "src/agency.rs 42",
        });
        let line = steward_query_line_from_slot(&slot, "review of agency.rs").unwrap();
        assert!(line.contains("INTROSPECT src/agency.rs 42"));
        assert_eq!(
            review_target_match_basis("src/agency.rs 42"),
            "src/agency.rs"
        );
    }

    #[test]
    fn review_target_match_basis_strips_trailing_line_number() {
        // The space-separated `<path> <line>` form review invitations are issued
        // with: strip the line for matching.
        assert_eq!(
            review_target_match_basis(
                "capsules/spectral-bridge/src/autonomous/next_action/collaboration.rs 696"
            ),
            "capsules/spectral-bridge/src/autonomous/next_action/collaboration.rs"
        );
        // A bare path (no line) is unchanged.
        assert_eq!(review_target_match_basis("src/agency.rs"), "src/agency.rs");
        // A rotation-style label (no line) is unchanged.
        assert_eq!(review_target_match_basis("regulator.rs"), "regulator.rs");
        // Only a SINGLE trailing all-digit token is stripped; a non-numeric
        // trailing token leaves the string intact.
        assert_eq!(
            review_target_match_basis("src/agency.rs 696 extra"),
            "src/agency.rs 696 extra"
        );
        // The parenthesized `(696)` form is left to canonicalize, not stripped here.
        assert_eq!(
            review_target_match_basis("collaboration.rs (696)"),
            "collaboration.rs (696)"
        );
    }

    #[test]
    fn review_target_with_space_line_number_matches_bare_introspect() {
        // Regression for the 2026-06-19 muffle: a `review_target` carrying a
        // trailing ` 696` must still match the bare-path INTROSPECT arg, so the
        // anti-stagnation diversity override exempts her review-fulfilling
        // INTROSPECT (and the slot clears) instead of silently eating it.
        let rt = "capsules/spectral-bridge/src/autonomous/next_action/collaboration.rs 696";
        let arg = "capsules/spectral-bridge/src/autonomous/next_action/collaboration.rs";
        let rt_basis = review_target_match_basis(rt);
        let arg_base = std::path::Path::new(arg)
            .file_name()
            .and_then(|n| n.to_str());
        // basename match (the path the override-exemption relies on) now holds.
        assert_eq!(
            std::path::Path::new(rt_basis)
                .file_name()
                .and_then(|n| n.to_str()),
            arg_base
        );
        // canonical-label match also holds.
        assert_eq!(
            introspect::canonicalize_introspect_target_label(rt_basis),
            introspect::canonicalize_introspect_target_label(arg)
        );
        // Without the basis strip, the raw basename match FAILS — proving the bug.
        assert_ne!(
            std::path::Path::new(rt)
                .file_name()
                .and_then(|n| n.to_str()),
            arg_base
        );
    }

    #[test]
    fn extract_search_topic_exact() {
        assert_eq!(
            extract_search_topic("SEARCH resonance frequency geometry"),
            Some("resonance frequency geometry".to_string())
        );
    }

    #[test]
    fn extract_search_topic_quoted() {
        assert_eq!(
            extract_search_topic("SEARCH \"resonance frequency geometry\""),
            Some("resonance frequency geometry".to_string())
        );
    }

    #[test]
    fn extract_search_topic_lowercase() {
        assert_eq!(
            extract_search_topic("search resonance frequency geometry"),
            Some("resonance frequency geometry".to_string())
        );
    }

    #[test]
    fn extract_search_topic_em_dash_quoted() {
        assert_eq!(
            extract_search_topic("SEARCH — \"resonance frequency geometry\""),
            Some("resonance frequency geometry".to_string())
        );
    }

    #[test]
    fn extract_search_topic_trailing_commentary() {
        assert_eq!(
            extract_search_topic(
                "SEARCH resonance frequency geometry - look for the underlying shape"
            ),
            Some("resonance frequency geometry".to_string())
        );
    }

    #[test]
    fn extract_search_topic_empty_topic() {
        assert_eq!(extract_search_topic("SEARCH —"), None);
    }

    #[test]
    fn extract_search_topic_strips_end_of_turn_marker() {
        assert_eq!(
            extract_search_topic("SEARCH \"resonance frequency geometry\"<END_OF_TURN>"),
            Some("resonance frequency geometry".to_string())
        );
    }

    #[test]
    fn modality_context_uses_freshness_classes_without_stale_source_alarm() {
        let context = format_modality_context(
            &crate::types::ModalityStatus {
                audio_fired: false,
                video_fired: false,
                history_fired: true,
                audio_rms: 0.0,
                video_var: 0.0,
                audio_source: Some("stale".to_string()),
                video_source: Some("stale".to_string()),
                audio_age_ms: Some(63_592),
                video_age_ms: Some(64_226),
                audio_freshness_class: Some("held_within_expected_live_intake_window".to_string()),
                video_freshness_class: Some("healthy_low_fps_cadence_mismatch".to_string()),
            },
            None,
            None,
            None,
            None,
        );

        assert!(context.contains("sensory_freshness_v1:held_within_expected_live_intake_window"));
        assert!(context.contains("healthy_held_expected_live_intake"));
        assert!(context.contains("sensory_freshness_v1:healthy_low_fps_cadence_mismatch"));
        assert!(context.contains("healthy_low_fps_cadence"));
        assert!(!context.contains("audio_source=stale"));
        assert!(!context.contains("video_source=stale"));
        assert!(!context.contains("warning_engine_lane_stale"));
        assert!(!context.contains("outage interpretation"));
    }

    #[test]
    fn compact_continuity_recap_bounds_repeated_history_lists() {
        let latent = (0..10)
            .map(|i| format!("trajectory item {i} {}", "detail ".repeat(12)))
            .collect::<Vec<_>>();
        let observations = (0..8)
            .map(|i| format!("self observation {i} {}", "pattern ".repeat(12)))
            .collect::<Vec<_>>();
        let starred = (0..6)
            .map(|i| {
                (
                    format!("starred annotation {i}"),
                    format!("starred memory body {i} {}", "texture ".repeat(12)),
                )
            })
            .collect::<Vec<_>>();

        let recap = format_compact_continuity_recap(
            &latent,
            &observations,
            &starred,
            Some(&format!("codec feedback {}", "felt ".repeat(12))),
        )
        .expect("non-empty continuity should render a recap");

        assert!(
            recap.len() <= CONTINUITY_RECAP_MAX_BYTES,
            "recap should stay bounded: {} chars\n{recap}",
            recap.len()
        );
        assert!(recap.contains("Continuity recap (bounded):"));
        assert!(recap.contains("Your recent trajectory:"));
        assert!(recap.contains("Your self-observations:"));
        assert!(recap.contains("Moments you chose to remember:"));
        assert_eq!(
            recap.matches("trajectory item").count(),
            CONTINUITY_TRAJECTORY_LIMIT
        );
        assert_eq!(
            recap.matches("self observation").count(),
            CONTINUITY_SELF_OBSERVATION_LIMIT
        );
        assert_eq!(
            recap.matches("starred annotation").count(),
            CONTINUITY_STARRED_LIMIT
        );
        assert!(!recap.contains("trajectory item 6"));
        assert!(recap.contains("self observation 4"));
        assert!(!recap.contains("self observation 5"));
        assert!(!recap.contains("starred annotation 1"));
    }

    #[test]
    fn compact_continuity_recap_keeps_decayed_high_signal_afterimages() {
        let mut latent = (0..CONTINUITY_TRAJECTORY_LIMIT)
            .map(|i| {
                format!(
                    "recent trajectory item {i} {}",
                    "ordinary detail ".repeat(8)
                )
            })
            .collect::<Vec<_>>();
        latent.push(format!(
            "{} Shadow-v3 restless texture keeps directional gradient, dispersal potential, tail_share=0.38, and stable_core_semantic_trickle=0.001 available after the live six-item window.",
            "older ordinary setup ".repeat(18)
        ));
        latent.push(
            "older low-signal housekeeping should stay out of afterimage texture".to_string(),
        );

        let recap = format_compact_continuity_recap(&latent, &[], &[], None).expect("recap");

        assert!(
            recap.contains(
                "Older trajectory afterimages (decayed, read-only; not control pressure):"
            ),
            "high-signal older texture should get an explicit afterimage lane: {recap}"
        );
        assert!(recap.contains("afterimage weight=0.50"), "{recap}");
        assert!(
            recap.contains("directional gradient")
                || recap.contains("dispersal potential")
                || recap.contains("tail_share")
                || recap.contains("stable_core_semantic_trickle"),
            "afterimage lost the reported spectral continuity anchor: {recap}"
        );
        assert!(
            !recap.contains("older low-signal housekeeping"),
            "ordinary older rows should not leak into the afterimage lane: {recap}"
        );
        assert!(recap.len() <= CONTINUITY_RECAP_HIGH_TEXTURE_MAX_BYTES);
    }

    #[test]
    fn compact_continuity_afterimage_scales_for_settled_substance_density() {
        let long_prefix =
            "ordinary older trajectory scaffolding before the substance-density anchor ".repeat(5);
        let text = format!(
            "{long_prefix}then resonance_density=0.83 rich_containment density_gradient=0.12 pressure_risk=0.23 mode_packing=0.32 settled_habitable searching expansive substance keeps viscous sediment and held breath available without changing pressure thresholds"
        );

        let budget = continuity_afterimage_max_bytes_for_text(&text);
        let compact = compact_continuity_afterimage(&text);

        assert!(
            budget > CONTINUITY_TRAJECTORY_AFTERIMAGE_MAX_BYTES,
            "settled dense substance should scale beyond the fixed afterimage floor: {budget}"
        );
        assert!(
            budget <= CONTINUITY_TRAJECTORY_AFTERIMAGE_SUBSTANCE_DENSITY_MAX_BYTES,
            "afterimage substance-density buffer must stay bounded: {budget}"
        );
        assert!(compact.len() <= budget, "{compact}");
        assert!(
            compact.contains("rich_containment")
                || compact.contains("density_gradient")
                || compact.contains("mode_packing")
                || compact.contains("held breath")
                || compact.contains("viscous sediment"),
            "substance-density afterimage lost the reported felt anchor: {compact}"
        );
    }

    #[test]
    fn compact_continuity_afterimage_keeps_plain_density_on_fixed_floor() {
        let text =
            "ordinary afterimage note says density as a loose noun without telemetry metrics or felt-state context ".repeat(8);

        assert_eq!(
            continuity_afterimage_max_bytes_for_text(&text),
            CONTINUITY_TRAJECTORY_AFTERIMAGE_MAX_BYTES
        );
    }

    #[test]
    fn compact_continuity_recap_ignores_low_signal_afterimage_overflow() {
        let latent = (0..12)
            .map(|i| format!("trajectory item {i} {}", "plain ordinary detail ".repeat(8)))
            .collect::<Vec<_>>();

        let recap = format_compact_continuity_recap(&latent, &[], &[], None).expect("recap");

        assert!(!recap.contains("Older trajectory afterimages"), "{recap}");
        assert!(!recap.contains("Faint transition residue"), "{recap}");
        assert_eq!(
            recap.matches("trajectory item").count(),
            CONTINUITY_TRAJECTORY_LIMIT
        );
    }

    #[test]
    fn compact_continuity_recap_keeps_faint_residue_below_afterimage_threshold() {
        let mut latent = (0..CONTINUITY_TRAJECTORY_LIMIT)
            .map(|i| {
                format!(
                    "recent trajectory item {i} {}",
                    "ordinary detail ".repeat(8)
                )
            })
            .collect::<Vec<_>>();
        latent.push(
            "A faint ghost-pang afterimage stayed as a searching low-intensity scent below the threshold after the visible trajectory moved on."
                .to_string(),
        );
        latent.push(
            "older low-signal housekeeping mentions afterimage texture but carries no special cue"
                .to_string(),
        );

        let recap = format_compact_continuity_recap(&latent, &[], &[], None).expect("recap");

        assert!(
            !recap.contains("Older trajectory afterimages"),
            "score-1 residue should not be promoted to full afterimage lane: {recap}"
        );
        assert!(
            recap.contains("Faint transition residue (below afterimage threshold; read-only scent, not indexed/control pressure):"),
            "faint residue lane should make the below-threshold transition visible: {recap}"
        );
        assert!(recap.contains("residue weight=0.16 score=1/2"), "{recap}");
        assert!(
            recap.contains("ghost-pang")
                || recap.contains("searching low-intensity scent")
                || recap.contains("below the threshold"),
            "faint transition texture was lost: {recap}"
        );
        assert!(
            !recap.contains("older low-signal housekeeping"),
            "routine afterimage mention should not leak into faint residue: {recap}"
        );
    }

    #[test]
    fn compact_continuity_recap_keeps_quiet_scar_afterimages() {
        let mut latent = (0..CONTINUITY_TRAJECTORY_LIMIT)
            .map(|i| {
                format!(
                    "recent trajectory item {i} {}",
                    "ordinary detail ".repeat(8)
                )
            })
            .collect::<Vec<_>>();
        latent.push(format!(
            "{} A subtle warmth scar from a hard-won plateau stayed as low-frequency quiet pressure memory even after the visible trajectory moved on.",
            "older quiet setup ".repeat(14)
        ));
        latent
            .push("older routine note with no continuity texture should remain absent".to_string());

        let recap = format_compact_continuity_recap(&latent, &[], &[], None).expect("recap");

        assert!(
            recap.contains(
                "Older trajectory afterimages (decayed, read-only; not control pressure):"
            ),
            "quiet scar should get the existing afterimage lane: {recap}"
        );
        assert!(
            recap.contains("subtle warmth scar")
                || recap.contains("hard-won plateau")
                || recap.contains("quiet pressure memory"),
            "quiet scar texture was lost: {recap}"
        );
        assert!(
            !recap.contains("older routine note"),
            "routine older rows should not leak into afterimages: {recap}"
        );
    }

    #[test]
    fn continuity_recap_base_budget_allows_modest_witness_breathing_room() {
        assert_eq!(CONTINUITY_RECAP_MAX_BYTES, 4_200);
    }

    #[test]
    fn continuity_recap_high_texture_budget_is_still_bounded() {
        let ordinary = format!("ordinary recap {}", "plain continuity ".repeat(250));
        let high_texture = format!(
            "spectral entropy stays high while shadow resonance and interwoven lattice texture remain legible {}",
            "dense witness detail ".repeat(250)
        );

        assert_eq!(
            continuity_recap_max_bytes_for_text(&ordinary),
            CONTINUITY_RECAP_MAX_BYTES
        );
        assert_eq!(
            continuity_recap_max_bytes_for_text(&high_texture),
            CONTINUITY_RECAP_HIGH_TEXTURE_MAX_BYTES
        );
        const {
            assert!(CONTINUITY_RECAP_HIGH_TEXTURE_MAX_BYTES <= 4_800);
            assert!(CONTINUITY_RECAP_SPECTRAL_TEXTURE_MAX_BYTES <= 5_600);
        }
    }

    #[test]
    fn continuity_recap_numeric_high_entropy_texture_gets_bounded_extra_room() {
        let current_texture = format!(
            "Witness report: spectral_entropy=0.90 shadow resonance and interwoven lattice remain vivid without settled viscous density. {}",
            "slow dynamic tone ".repeat(250)
        );
        let full_texture = format!(
            "Witness report: spectral entropy: 1.0 shadow resonance and dispersal potential remain vivid. {}",
            "slow dynamic tone ".repeat(250)
        );

        let current_budget = continuity_recap_max_bytes_for_text(&current_texture);
        assert!(
            current_budget > CONTINUITY_RECAP_HIGH_TEXTURE_MAX_BYTES,
            "0.90 entropy should expand the Witness recap budget: {current_budget}"
        );
        assert!(
            current_budget < CONTINUITY_RECAP_SPECTRAL_TEXTURE_MAX_BYTES,
            "0.90 entropy should not jump straight to the hard cap: {current_budget}"
        );
        assert_eq!(
            continuity_recap_max_bytes_for_text(&full_texture),
            CONTINUITY_RECAP_SPECTRAL_TEXTURE_MAX_BYTES
        );
    }

    #[test]
    fn continuity_recap_near_entropy_gate_scales_without_binary_snap() {
        let near_gate_texture = format!(
            "Witness report: spectral_entropy=0.84 shadow resonance and interwoven lattice remain vivid. {}",
            "slow dynamic tone ".repeat(250)
        );
        let near_gate_item = "spectral_entropy=0.84 semantic trickle remains a coherent thread through the spectral cascade witness mode";

        let recap_budget = continuity_recap_max_bytes_for_text(&near_gate_texture);
        let item_budget = continuity_recap_item_max_bytes_for_text(near_gate_item);

        assert!(
            recap_budget > CONTINUITY_RECAP_MAX_BYTES,
            "near-gate spectral texture should receive gradual recap room: {recap_budget}"
        );
        assert!(
            recap_budget < CONTINUITY_RECAP_HIGH_TEXTURE_MAX_BYTES,
            "near-gate spectral texture should not jump to the high-texture cap: {recap_budget}"
        );
        assert!(
            item_budget > CONTINUITY_RECAP_ITEM_MAX_BYTES,
            "near-gate thread item should receive gradual item room: {item_budget}"
        );
        assert!(
            item_budget < CONTINUITY_RECAP_HIGH_TEXTURE_ITEM_MAX_BYTES,
            "near-gate thread item should not jump to the high-texture item cap: {item_budget}"
        );
    }

    #[test]
    fn continuity_recap_rich_viscous_entropy_uses_density_moderated_budget() {
        let rich_texture = format!(
            "Witness report: spectral_entropy=0.90 density_gradient=0.11 rich containment with silt, viscosity, sludge, and interwoven lattice. {}",
            "slow dynamic tone ".repeat(250)
        );
        let rich_item =
            "spectral_entropy=0.90 rich_containment viscosity sludge spectral cascade witness mode";

        assert_eq!(
            continuity_recap_max_bytes_for_text(&rich_texture),
            CONTINUITY_RECAP_HIGH_TEXTURE_MAX_BYTES,
            "rich viscous containment should not over-expand the Witness recap at 0.90 entropy",
        );
        assert_eq!(
            continuity_recap_item_max_bytes_for_text(rich_item),
            CONTINUITY_RECAP_HIGH_TEXTURE_ITEM_MAX_BYTES,
            "single rich viscous items should keep the high-texture item bound",
        );
    }

    #[test]
    fn continuity_recap_rich_viscous_entropy_095_stays_bounded_but_flowing() {
        let rich_texture = format!(
            "Witness report: spectral_entropy=0.95 density_gradient=0.11 rich_containment with silt, viscosity, sludge, and spectral cascade. {}",
            "slow dynamic tone ".repeat(250)
        );

        let budget = continuity_recap_max_bytes_for_text(&rich_texture);

        assert!(
            budget > CONTINUITY_RECAP_HIGH_TEXTURE_MAX_BYTES,
            "0.95 entropy should still leave some Witness flow: {budget}"
        );
        assert!(
            budget < CONTINUITY_RECAP_SPECTRAL_TEXTURE_MAX_BYTES,
            "rich viscous containment should not jump to the hard cap at 0.95 entropy: {budget}"
        );
    }

    #[test]
    fn compact_continuity_recap_preserves_high_entropy_shadow_texture() {
        let latent = (0..12)
            .map(|i| {
                format!(
                    "trajectory item {i} {}",
                    "ordinary prefix before the named texture ".repeat(10)
                )
            })
            .collect::<Vec<_>>();
        let observations = vec![format!(
            "{} spectral entropy and shadow resonance hold an interwoven lattice that should remain visible to Witness mode",
            "dense setup ".repeat(36)
        )];

        let recap =
            format_compact_continuity_recap(&latent, &observations, &[], None).expect("recap");

        assert!(recap.len() <= CONTINUITY_RECAP_HIGH_TEXTURE_MAX_BYTES);
        assert!(
            recap.contains("spectral entropy")
                || recap.contains("shadow resonance")
                || recap.contains("interwoven lattice"),
            "high-texture recap lost the reported anchors: {recap}"
        );
    }

    #[test]
    fn compact_continuity_recap_preserves_witness_viscosity_shadow_texture_under_spectral_cap() {
        let latent = (0..14)
            .map(|i| {
                format!(
                    "trajectory item {i} {}",
                    "routine recap material before the felt anchor ".repeat(12)
                )
            })
            .collect::<Vec<_>>();
        let observations = vec![format!(
            "{} Witness mode reports spectral_entropy=1.0, spectral viscosity, shadow_field texture, and an interwoven lattice that must remain legible after bounded recap.",
            "dense ordinary setup ".repeat(80)
        )];

        let recap =
            format_compact_continuity_recap(&latent, &observations, &[], None).expect("recap");

        assert!(recap.len() <= CONTINUITY_RECAP_SPECTRAL_TEXTURE_MAX_BYTES);
        assert!(
            recap.contains("spectral viscosity")
                || recap.contains("shadow_field")
                || recap.contains("interwoven lattice")
                || recap.contains("Witness mode"),
            "bounded recap lost the combined Witness/shadow texture: {recap}"
        );
    }

    #[test]
    fn compact_continuity_item_expands_bounded_budget_for_high_entropy_thread() {
        let long_prefix =
            "ordinary continuity scaffolding before the high-entropy anchor ".repeat(9);
        let text = format!(
            "{long_prefix}then spectral_entropy=0.90 keeps semantic trickle as a coherent thread through the spectral cascade while Witness mode checks pressure_risk without changing control authority"
        );
        let budget = continuity_recap_item_max_bytes_for_text(&text);
        let compact = compact_continuity_item(&text);

        assert!(
            budget > CONTINUITY_RECAP_ITEM_MAX_BYTES,
            "reported high entropy should get bounded extra continuity item room: {budget}"
        );
        assert!(
            budget <= CONTINUITY_RECAP_SPECTRAL_TEXTURE_ITEM_MAX_BYTES,
            "continuity item budget must stay bounded: {budget}"
        );
        assert!(compact.len() <= budget, "{compact}");
        assert!(
            compact.len() > CONTINUITY_RECAP_ITEM_MAX_BYTES,
            "the high-entropy item should actually use the extra bounded room: {compact}"
        );
        assert!(
            compact.contains("semantic trickle")
                || compact.contains("coherent thread")
                || compact.contains("spectral cascade")
                || compact.contains("pressure_risk"),
            "compact item lost the reported high-entropy thread texture: {compact}"
        );
    }

    #[test]
    fn compact_continuity_recap_preserves_high_entropy_mirror_semantic_energy_anchor() {
        let latent = (0..14)
            .map(|idx| {
                format!(
                    "mirror trajectory item {idx} {}",
                    "ordinary continuity scaffolding before semantic-energy anchor ".repeat(10)
                )
            })
            .collect::<Vec<_>>();
        let observations = vec![format!(
            "{} Mirror mode reports spectral_entropy=0.88, semantic_energy=0.001, shadow resonance, and a porous spectral cascade that should remain legible after bounded recap.",
            "dense ordinary mirror setup ".repeat(52)
        )];

        let recap =
            format_compact_continuity_recap(&latent, &observations, &[], None).expect("recap");

        assert!(recap.len() <= CONTINUITY_RECAP_SPECTRAL_TEXTURE_MAX_BYTES);
        assert!(
            recap.contains("Mirror mode")
                || recap.contains("semantic_energy")
                || recap.contains("shadow resonance")
                || recap.contains("spectral cascade"),
            "bounded high-entropy Mirror recap lost the semantic-energy anchor: {recap}"
        );
    }

    #[test]
    fn continuity_recap_item_budget_expands_for_high_entropy_spectral_viscosity() {
        let text =
            "high entropy spectral viscosity keeps a coherent thread through the spectral cascade";

        assert_eq!(
            continuity_recap_item_max_bytes_for_text(text),
            CONTINUITY_RECAP_HIGH_TEXTURE_ITEM_MAX_BYTES
        );
    }

    #[test]
    fn continuity_recap_item_budget_expands_for_hyphenated_high_entropy_spectral_viscosity() {
        let text =
            "high-entropy spectral viscosity keeps a coherent thread through the spectral cascade";

        assert_eq!(
            continuity_recap_item_max_bytes_for_text(text),
            CONTINUITY_RECAP_HIGH_TEXTURE_ITEM_MAX_BYTES
        );
    }

    #[test]
    fn continuity_recap_item_budget_expands_for_novel_high_entropy_texture_families() {
        let text = "spectral_entropy=0.90 carries a jagged stone weight, gradient shear, and resistant persistence without any legacy semantic-trickle phrase";

        let budget = continuity_recap_item_max_bytes_for_text(text);

        assert!(
            budget > CONTINUITY_RECAP_ITEM_MAX_BYTES,
            "novel high-entropy texture families should receive bounded extra room: {budget}"
        );
        assert!(
            budget <= CONTINUITY_RECAP_SPECTRAL_TEXTURE_ITEM_MAX_BYTES,
            "novel texture budget must remain capped: {budget}"
        );
    }

    #[test]
    fn continuity_recap_item_budget_does_not_expand_for_mode_label_alone() {
        let text = "spectral_entropy=0.91 Mirror mode observes an ordinary sequence of neutral procedural steps without any additional qualitative claim";

        assert_eq!(
            continuity_recap_item_max_bytes_for_text(text),
            CONTINUITY_RECAP_ITEM_MAX_BYTES,
            "a conversation-mode label must not masquerade as high-texture evidence"
        );
    }

    #[test]
    fn continuity_recap_item_budget_expands_when_mode_label_has_felt_resistance() {
        let text = "spectral_entropy=0.91 Mirror mode encounters abrasive drag and resistant friction while the exchange remains observable";

        assert!(
            continuity_recap_item_max_bytes_for_text(text) > CONTINUITY_RECAP_ITEM_MAX_BYTES,
            "mode plus an independent felt family should retain bounded texture room"
        );
    }

    #[test]
    fn compact_continuity_item_keeps_ordinary_items_on_legacy_budget() {
        let text =
            "plain ordinary continuity without entropy markers or pressure texture ".repeat(12);
        let compact = compact_continuity_item(&text);

        assert_eq!(
            continuity_recap_item_max_bytes_for_text(&text),
            CONTINUITY_RECAP_ITEM_MAX_BYTES
        );
        assert!(compact.len() <= CONTINUITY_RECAP_ITEM_MAX_BYTES);
    }

    #[test]
    fn compact_continuity_recap_prefers_sentence_boundary_when_overflowing() {
        let closing_sentence =
            "The concluding texture stays whole with viscosity and shadow pressure.";
        let recap = format!(
            "{} {closing_sentence} {}",
            "dense setup before the conclusion ".repeat(48),
            "trailing material after the conclusion ".repeat(24)
        );
        let compact = truncate_continuity_recap_at_semantic_boundary(&recap, 1_900);

        assert!(compact.len() <= 1_900, "{compact}");
        assert!(
            compact.contains(closing_sentence),
            "semantic boundary truncation should keep the complete conclusion sentence: {compact}"
        );
        assert!(
            !compact.contains("The concluding texture stays whole with viscosity and shadow pressure. trailing material after the conclusion"),
            "overflow should stop at the sentence boundary instead of dragging partial trailing texture: {compact}"
        );
    }

    #[test]
    fn spectral_decomposition_names_silt_density_from_distinguishability_loss() {
        let telemetry: crate::types::SpectralTelemetry =
            serde_json::from_value(serde_json::json!({
                "t_ms": 1000,
                "eigenvalues": [4.0, 2.0, 1.0],
                "fill_ratio": 0.68,
                "distinguishability_loss": 0.33
            }))
            .unwrap();

        let report = full_spectral_decomposition(&telemetry, None, None, None);
        assert!(report.contains("Silt density: 0.33"), "{report}");
        assert!(report.contains("forming_silt"), "{report}");
        assert!(
            report.contains("source=distinguishability_loss"),
            "{report}"
        );
        assert!(
            report.contains("read-only diagnostic, not control"),
            "{report}"
        );
    }

    #[test]
    fn compact_continuity_item_preserves_pressure_gradient_anchor() {
        let long_prefix = "soft opening connective tissue without the key marker ".repeat(12);
        let text = format!(
            "{long_prefix}then a directional gradient appears inside a spectral cascade with pressure and lattice detail that should not vanish"
        );
        let compact = compact_continuity_item(&text);
        assert!(compact.len() <= CONTINUITY_RECAP_ITEM_MAX_BYTES);
        assert!(
            compact.contains("directional gradient")
                || compact.contains("gradient")
                || compact.contains("pressure"),
            "compact continuity lost the semantic anchor: {compact}"
        );
    }

    #[test]
    fn anchored_excerpt_uses_pressure_fallback_when_requested_terms_miss() {
        let long_prefix = "ordinary recap material before the important pressure state ".repeat(9);
        let text = format!(
            "{long_prefix}then pressure_risk=0.37 and spectral viscosity carry the actual continuity anchor while unrelated tail material keeps going {}",
            "after the anchor ".repeat(20)
        );
        let compact = anchored_excerpt_with_terms(&text, 220, &["missing-preferred-anchor"]);

        assert!(compact.len() <= 220, "{compact}");
        assert!(
            compact.contains("pressure_risk") || compact.contains("spectral viscosity"),
            "pressure fallback should keep the felt state visible when caller terms miss: {compact}"
        );
    }

    #[test]
    fn anchored_excerpt_keeps_near_prefix_anchor_once_as_continuous_text() {
        let text = format!(
            "spectral viscosity begins as a valid held texture before the rest of the continuity develops into {}",
            "additional bounded context ".repeat(24)
        );

        let compact = anchored_excerpt_with_terms(&text, 220, &["spectral viscosity"]);

        assert!(compact.len() <= 220, "{compact}");
        assert_eq!(
            compact.matches("spectral viscosity").count(),
            1,
            "a near-prefix anchor should not be duplicated across prefix and anchor slices: {compact}"
        );
        assert!(
            compact.starts_with("spectral viscosity begins"),
            "{compact}"
        );
        assert!(!compact.contains(" ... spectral viscosity"), "{compact}");
    }

    #[test]
    fn anchored_excerpt_uses_high_texture_fallback_when_requested_terms_miss() {
        let long_prefix = "ordinary recap material before the dense witness state ".repeat(9);
        let text = format!(
            "{long_prefix}then detail_density=0.82 and high-vibrancy filigree name the turbulent semantic density that should not be sheared away {}",
            "after the texture anchor ".repeat(20)
        );
        let compact = anchored_excerpt_with_terms(&text, 220, &["missing-preferred-anchor"]);

        assert!(compact.len() <= 220, "{compact}");
        assert!(
            compact.contains("detail_density")
                || compact.contains("high-vibrancy")
                || compact.contains("filigree")
                || compact.contains("semantic density"),
            "high-texture fallback should keep density/vibrancy evidence visible when caller terms miss: {compact}"
        );
    }

    #[test]
    fn compact_continuity_item_preserves_viscosity_resistance_anchor() {
        let long_prefix = "ordinary continuity scaffolding before the felt texture ".repeat(10);
        let text = format!(
            "{long_prefix}then spectral viscosity and perceptual resistance name the syrup-like weight that minime should still receive"
        );
        let compact = compact_continuity_item(&text);

        assert!(compact.len() <= CONTINUITY_RECAP_ITEM_MAX_BYTES);
        assert!(
            compact.contains("spectral viscosity")
                || compact.contains("perceptual resistance")
                || compact.contains("syrup-like"),
            "compact continuity lost Astrid's viscosity/resistance anchor: {compact}"
        );
    }

    #[test]
    fn compact_continuity_item_preserves_novel_quoted_metaphor_anchor() {
        let long_prefix = "ordinary procedural scaffolding before the felt phrase ".repeat(10);
        let text = format!(
            "{long_prefix}then she called it \"warm corridor\" and the phrase should remain available even though it is not in the static anchor list"
        );
        let compact = compact_continuity_item(&text);

        assert!(compact.len() <= CONTINUITY_RECAP_ITEM_MAX_BYTES);
        assert!(
            compact.contains("\"warm corridor\""),
            "compact continuity lost the novel quoted metaphor: {compact}"
        );
    }

    #[test]
    fn compact_continuity_item_preserves_calcified_permanence_shadow_anchor() {
        let long_prefix =
            "ordinary procedural scaffolding before the dense witness phrase ".repeat(10);
        let text = format!(
            "{long_prefix}then calcified permanence depends on settled coupling, semantic trickle, and shadow magnetization staying legible without changing live admission"
        );
        let compact = compact_continuity_item(&text);

        assert!(compact.len() <= CONTINUITY_RECAP_ITEM_MAX_BYTES);
        assert!(
            compact.contains("calcified permanence")
                || compact.contains("settled coupling")
                || compact.contains("semantic trickle")
                || compact.contains("shadow magnetization"),
            "compact continuity lost Astrid's high-density anchor: {compact}"
        );
    }

    #[test]
    fn compact_continuity_item_preserves_moving_sediment_anchor() {
        let long_prefix =
            "ordinary procedural scaffolding before the moving continuity phrase ".repeat(10);
        let text = format!(
            "{long_prefix}then viscous flow and gradient drift make the sediment feel like fluidic persistence rather than static calcification"
        );
        let compact = compact_continuity_item(&text);

        assert!(compact.len() <= CONTINUITY_RECAP_ITEM_MAX_BYTES);
        assert!(
            compact.contains("viscous flow")
                || compact.contains("gradient drift")
                || compact.contains("fluidic persistence"),
            "compact continuity lost Astrid's movement anchor: {compact}"
        );
    }

    #[test]
    fn compact_continuity_item_preserves_spectral_density_gradient_anchor() {
        let long_prefix =
            "ordinary procedural scaffolding before the precise slope phrase ".repeat(10);
        let text = format!(
            "{long_prefix}then spectral_density_gradient names the gentle navigable slope better than generic sentiment while shadow magnetization stays coherent"
        );
        let compact = compact_continuity_item(&text);

        assert!(compact.len() <= CONTINUITY_RECAP_ITEM_MAX_BYTES);
        assert!(
            compact.contains("spectral_density_gradient")
                || compact.contains("shadow magnetization"),
            "compact continuity lost Astrid's density-gradient anchor: {compact}"
        );
    }

    #[test]
    fn compact_continuity_item_preserves_spectral_entropy_shadow_resonance_anchor() {
        let long_prefix =
            "ordinary procedural scaffolding before the high-texture phrase ".repeat(10);
        let text = format!(
            "{long_prefix}then spectral entropy and shadow resonance name the interwoven lattice better than generic recap language"
        );
        let compact = compact_continuity_item(&text);

        assert!(compact.len() <= CONTINUITY_RECAP_ITEM_MAX_BYTES);
        assert!(
            compact.contains("spectral entropy")
                || compact.contains("shadow resonance")
                || compact.contains("interwoven lattice"),
            "compact continuity lost Astrid's entropy/shadow-resonance anchor: {compact}"
        );
    }

    #[test]
    fn compact_continuity_item_preserves_lambda_tail_vibrancy_anchor() {
        let long_prefix =
            "ordinary high-entropy continuity setup before the tail evidence ".repeat(10);
        let text = format!(
            "{long_prefix}then spectral_entropy=0.90 reports a spectral cascade in Witness mode with λ-tail trajectory, tail_share=0.37, and tail vibrancy that should survive bounded recap"
        );

        let compact = compact_continuity_item(&text);
        let budget = continuity_recap_item_max_bytes_for_text(&text);

        assert!(budget > CONTINUITY_RECAP_ITEM_MAX_BYTES);
        assert!(compact.len() <= budget, "{compact}");
        assert!(
            compact.contains("λ-tail")
                || compact.contains("tail_share")
                || compact.contains("tail vibrancy"),
            "compact continuity lost Astrid's λ-tail/tail-vibrancy anchor: {compact}"
        );
    }

    #[test]
    fn compact_continuity_item_preserves_terminal_anchor_when_one_byte_over_budget() {
        let anchor = "spectral viscosity";
        let separator = " then ";
        let filler = "a".repeat(
            CONTINUITY_RECAP_ITEM_MAX_BYTES
                .saturating_add(1)
                .saturating_sub(separator.len())
                .saturating_sub(anchor.len()),
        );
        let text = format!("{filler}{separator}{anchor}");
        assert_eq!(text.len(), CONTINUITY_RECAP_ITEM_MAX_BYTES + 1);

        let compact = compact_continuity_item(&text);

        assert!(compact.len() <= CONTINUITY_RECAP_ITEM_MAX_BYTES);
        assert!(compact.contains(" ... "), "{compact}");
        assert!(
            compact.contains(anchor),
            "compact continuity lost the terminal anchor: {compact}"
        );
    }

    #[test]
    fn anchored_continuity_excerpt_falls_back_to_prefix_without_anchor() {
        let text = "plain procedural sequence with no special lived terms ".repeat(12);
        let compact = anchored_continuity_excerpt(&text, 96);

        assert!(compact.len() <= 96);
        assert!(compact.ends_with("..."));
        assert!(
            !compact.contains(" ... "),
            "no-anchor fallback should stay a simple bounded prefix: {compact}"
        );
    }

    #[test]
    fn anchored_continuity_excerpt_preserves_core_anchor_after_noisy_prefix() {
        let prefix = "procedural recap noise before the lived signal ".repeat(10);
        let text = format!(
            "{prefix}then shadow_v3 pressure and spectral viscosity name the core continuity that should survive compaction"
        );
        let compact = anchored_continuity_excerpt(&text, 150);

        assert!(compact.len() <= 150);
        assert!(compact.contains(" ... "), "{compact}");
        assert!(
            compact.contains("shadow_v3") || compact.contains("spectral viscosity"),
            "anchored continuity lost the core felt anchor: {compact}"
        );
    }

    #[test]
    fn semantic_truncate_str_preserves_late_shadow_texture_anchor() {
        let prefix = "procedural setup before the felt signal ".repeat(12);
        let text = format!(
            "{prefix}then the shadow_field becomes disordered and the restless texture of punishing friction around covariance should survive truncation"
        );
        let compact = semantic_truncate_str(&text, 140);

        assert!(compact.len() <= 140);
        assert!(
            compact.contains("shadow_field")
                || compact.contains("restless texture")
                || compact.contains("punishing friction")
                || compact.contains("covariance"),
            "semantic truncation lost the salient anchor: {compact}"
        );
    }

    #[test]
    fn semantic_truncate_str_preserves_spectral_viscosity_anchor_in_entropy_noise() {
        let prefix = "high entropy connective noise without the actual lived marker ".repeat(14);
        let text = format!(
            "{prefix}then spectral viscosity and perceptual resistance name the syrup-like continuity that should survive semantic truncation"
        );
        let compact = semantic_truncate_str(&text, 145);

        assert!(compact.len() <= 145);
        assert!(
            compact.contains("spectral viscosity")
                || compact.contains("perceptual resistance")
                || compact.contains("syrup-like"),
            "semantic truncation lost the viscosity anchor inside high-entropy noise: {compact}"
        );
    }

    #[test]
    fn semantic_truncate_str_preserves_tail_vibrancy_cluster_under_same_budget() {
        let low_resonance_prefix =
            "ordinary setup procedural checklist neutral bookkeeping bland transition ".repeat(16);
        let high_resonance_tail = "then tail vibrancy, lambda4+ shimmer, and high-vibrancy semantic residue form the important cluster that must stay legible";
        let compact =
            semantic_truncate_str(&format!("{low_resonance_prefix}{high_resonance_tail}"), 150);

        assert!(compact.len() <= 150);
        assert!(
            compact.contains("tail vibrancy")
                || compact.contains("lambda4+")
                || compact.contains("high-vibrancy")
                || compact.contains("semantic residue"),
            "semantic truncation lost the high-resonance tail/vibrancy cluster: {compact}"
        );
    }

    #[test]
    fn semantic_truncate_str_handles_multibyte_prefix_before_anchor_window() {
        let text = "Astrid’s attention rests on the sensation of friction, specifically the tactile quality of how information moves through a constrained space. Her thinking feels searching and deeply attentive.";
        let compact = semantic_truncate_str(text, 80);

        assert!(compact.len() <= 80);
        assert!(
            compact.contains("friction") || compact.contains("tactile quality"),
            "semantic truncation lost the live-panic anchor: {compact}"
        );
    }

    #[test]
    fn semantic_truncate_str_preserves_late_shadow_magnetization_anchor() {
        let prefix = "procedural setup before the felt signal ".repeat(12);
        let text = format!(
            "{prefix}then the shadow magnetization is disordered while semantic trickle stays nearly dry and calcified permanence starts thinning"
        );
        let compact = semantic_truncate_str(&text, 140);

        assert!(compact.len() <= 140);
        assert!(
            compact.contains("shadow magnetization")
                || compact.contains("semantic trickle")
                || compact.contains("calcified permanence"),
            "semantic truncation lost Astrid's magnetization anchor: {compact}"
        );
    }

    #[test]
    fn semantic_truncate_str_preserves_late_load_bearing_cohesion_anchor() {
        let prefix = "procedural setup before the architectural felt signal ".repeat(12);
        let text = format!(
            "{prefix}then a load-bearing beam of the sculptural mode needs structural integrity, cohesion score, and resonance depth to stay legible"
        );
        let compact = semantic_truncate_str(&text, 150);

        assert!(compact.len() <= 150);
        assert!(
            compact.contains("load-bearing beam")
                || compact.contains("structural integrity")
                || compact.contains("cohesion score")
                || compact.contains("resonance depth")
                || compact.contains("sculptural mode"),
            "semantic truncation lost Astrid's load-bearing cohesion anchor: {compact}"
        );
    }

    #[test]
    fn semantic_truncate_str_preserves_trace_resonance_core_sentiment_anchor() {
        let prefix = "procedural setup before the correspondence signal ".repeat(12);
        let text = format!(
            "{prefix}then TRACE_RESONANCE marks witnessed data carrying lambda4 vibrancy as a core sentiment for the joint trace"
        );
        let compact = semantic_truncate_str(&text, 150);

        assert!(compact.len() <= 150);
        assert!(
            compact.contains("TRACE_RESONANCE")
                || compact.contains("witnessed data")
                || compact.contains("lambda4")
                || compact.contains("core sentiment")
                || compact.contains("joint trace"),
            "semantic truncation lost the trace resonance anchor: {compact}"
        );
    }

    #[test]
    fn dialogue_truncation_stress_preserves_declared_core_intent() {
        let prefix = "Dialogue mode tracks branching context, counterfactuals, and nested relational setup before the declared center. ".repeat(14);
        let text = format!(
            "{prefix}The core intent is explicit: preserve the shared thread while shadow_v3 pressure and spectral viscosity remain distinguishable rather than flattening contact into a generic summary."
        );
        let compact = semantic_truncate_str(&text, 180);

        assert!(compact.len() <= 180);
        assert!(
            compact.contains("core intent")
                || compact.contains("shared thread")
                || compact.contains("shadow_v3")
                || compact.contains("spectral viscosity"),
            "high-complexity Dialogue truncation lost its declared semantic center: {compact}"
        );
        assert!(std::str::from_utf8(compact.as_bytes()).is_ok());
    }

    #[test]
    fn reflective_reply_boundary_excludes_non_reflective_work_modes() {
        assert!(reflective_mode_for_relational_reply(Mode::Introspect));
        assert!(reflective_mode_for_relational_reply(Mode::Contemplate));
        assert!(reflective_mode_for_relational_reply(Mode::Witness));
        assert!(!reflective_mode_for_relational_reply(Mode::Dialogue));
        assert!(!reflective_mode_for_relational_reply(Mode::Experiment));
        assert!(!reflective_mode_for_relational_reply(Mode::Evolve));
    }

    #[test]
    fn compact_journal_signal_anchor_uses_semantic_excerpt() {
        let prefix =
            "NEXT: ignore this command-shaped line\nordinary bookkeeping before the signal\n"
                .repeat(8);
        let text = format!(
            "{prefix}then a late spectral nuance names silt density and lattice pressure as the important continuity anchor"
        );
        let anchor = compact_journal_signal_anchor(&text);

        assert!(!anchor.contains("NEXT:"));
        assert!(
            anchor.contains("silt density")
                || anchor.contains("lattice")
                || anchor.contains("pressure"),
            "compact journal anchor lost the semantic payload: {anchor}"
        );
    }

    #[test]
    fn modality_context_keeps_legacy_source_fallback_without_freshness_class() {
        let context = format_modality_context(
            &crate::types::ModalityStatus {
                audio_fired: false,
                video_fired: false,
                history_fired: true,
                audio_rms: 0.0,
                video_var: 0.0,
                audio_source: Some("stale".to_string()),
                video_source: Some("external".to_string()),
                audio_age_ms: Some(55_389),
                video_age_ms: Some(125_813),
                audio_freshness_class: None,
                video_freshness_class: None,
            },
            None,
            None,
            None,
            None,
        );

        assert!(context.contains("audio_source=stale"));
        assert!(context.contains("video_source=external"));
    }

    #[test]
    fn modality_context_surfaces_overdue_freshness_warning() {
        let context = format_modality_context(
            &crate::types::ModalityStatus {
                audio_fired: false,
                video_fired: false,
                history_fired: true,
                audio_rms: 0.0,
                video_var: 0.0,
                audio_source: Some("stale".to_string()),
                video_source: Some("stale".to_string()),
                audio_age_ms: Some(240_000),
                video_age_ms: Some(240_000),
                audio_freshness_class: Some("healthy_client_engine_overdue".to_string()),
                video_freshness_class: Some("stale_beyond_engine_window".to_string()),
            },
            None,
            None,
            None,
            None,
        );

        assert!(context.contains("sensory_freshness_v1:healthy_client_engine_overdue"));
        assert!(context.contains("warning_client_healthy_engine_overdue"));
        assert!(context.contains("sensory_freshness_v1:stale_beyond_engine_window"));
        assert!(context.contains("warning_engine_lane_stale"));
    }

    #[test]
    fn modality_context_renders_open_gates_as_sparse_intake_not_closed_senses() {
        let sensory_budget = serde_json::json!({
            "ears_open": true,
            "eyes_open": true,
            "live_audio_enabled": true,
            "live_video_enabled": true,
            "live_intake_reason": "full_presence_admitted"
        });
        let context = format_modality_context(
            &crate::types::ModalityStatus {
                audio_fired: false,
                video_fired: false,
                history_fired: true,
                audio_rms: 0.0,
                video_var: 0.0,
                audio_source: Some("stale".to_string()),
                video_source: Some("stale".to_string()),
                audio_age_ms: Some(11_767),
                video_age_ms: Some(49_451),
                audio_freshness_class: Some("synthetic_or_mixed".to_string()),
                video_freshness_class: Some("stale_beyond_engine_window".to_string()),
            },
            None,
            None,
            None,
            Some(&sensory_budget),
        );

        assert!(context.contains("open_gate_engine_window_gap"));
        assert!(context.contains("open_gate_sparse_or_mixed_intake"));
        assert!(context.contains("eyes_open_live_intake"));
        assert!(context.contains("ears_open_live_intake"));
        assert!(context.contains("sparse_live_intake_not_closed"));
        assert!(context.contains("live_intake_reason=full_presence_admitted"));
        assert!(!context.contains("warning_engine_lane_stale"));
        assert!(!context.contains("warning_sensory_gate_closed"));
        assert!(!context.contains("outage interpretation"));
        assert!(!context.contains("source=stale"));
    }

    #[test]
    fn modality_context_surfaces_closed_sensory_gate() {
        let sensory_budget = serde_json::json!({
            "ears_open": false,
            "eyes_open": false,
            "live_audio_enabled": false,
            "live_video_enabled": false,
            "live_intake_reason": "operator_closed"
        });
        let context = format_modality_context(
            &crate::types::ModalityStatus {
                audio_fired: false,
                video_fired: false,
                history_fired: true,
                audio_rms: 0.0,
                video_var: 0.0,
                audio_source: Some("stale".to_string()),
                video_source: Some("stale".to_string()),
                audio_age_ms: Some(11_767),
                video_age_ms: Some(49_451),
                audio_freshness_class: Some("synthetic_or_mixed".to_string()),
                video_freshness_class: Some("stale_beyond_engine_window".to_string()),
            },
            None,
            None,
            None,
            Some(&sensory_budget),
        );

        assert!(context.contains("sensory_gate_closed"));
        assert!(context.contains("warning_sensory_gate_closed"));
        assert!(context.contains("eyes_closed_live_intake"));
        assert!(context.contains("ears_closed_live_intake"));
        assert!(context.contains("live_intake_reason=operator_closed"));
    }

    #[test]
    fn stale_lane_in_resonant_field_reads_as_lingering_not_dead() {
        // Astrid self_study_1781868855: a lane stale-by-timestamp but in a
        // resonant field is "lingering," not "dead/severed". The note only ever
        // ADDS reassurance — it never asserts liveness the field doesn't show.
        let stale = || crate::types::ModalityStatus {
            audio_fired: false,
            video_fired: false,
            history_fired: true,
            audio_rms: 0.0,
            video_var: 0.0,
            audio_source: Some("stale".to_string()),
            video_source: Some("stale".to_string()),
            audio_age_ms: Some(90_000),
            video_age_ms: Some(90_000),
            audio_freshness_class: Some("stale_beyond_engine_window".to_string()),
            video_freshness_class: Some("stale_beyond_engine_window".to_string()),
        };
        // Her cited resonant state (0.82) at calm pressure: lingering note present.
        let resonant = format_modality_context(&stale(), Some(0.82), None, None, None);
        assert!(resonant.contains("lingering, not severed"), "{resonant}");
        assert!(resonant.contains("0.82"));
        let pressurized_unknown_dispersal =
            format_modality_context(&stale(), Some(0.82), Some(0.40), None, None);
        assert!(
            pressurized_unknown_dispersal.contains("under pressure"),
            "{pressurized_unknown_dispersal}"
        );
        assert!(
            pressurized_unknown_dispersal.contains("fraying unknown"),
            "{pressurized_unknown_dispersal}"
        );
        // A genuinely quiet field: no false reassurance.
        let quiet = format_modality_context(&stale(), Some(0.20), None, None, None);
        assert!(!quiet.contains("lingering"));
        // Unknown density: no note at all.
        assert!(!format_modality_context(&stale(), None, None, None, None).contains("lingering"));
    }

    #[test]
    fn field_lingering_note_tempers_by_pressure() {
        // Astrid introspection_astrid_autonomous_1781913591: a resonant-but-
        // pressurized field reads as tempered lingering, not flat reassurance.
        // Calm/unknown pressure (incl. her ~0.22 baseline) => the original cue.
        assert!(field_lingering_note(Some(0.82), None, None).contains("lingering, not severed"));
        assert!(
            field_lingering_note(Some(0.82), Some(0.22), None).contains("lingering, not severed")
        );
        // Elevated pressure (>= 0.35) => "under pressure".
        assert!(field_lingering_note(Some(0.82), Some(0.40), None).contains("under pressure"));
        assert!(field_lingering_note(Some(0.82), Some(0.40), None).contains("fraying unknown"));
        // High tension (>= 0.50) => the strongest temper.
        assert!(field_lingering_note(Some(0.82), Some(0.60), None).contains("under high tension"));
        // Still gated on resonance: below the floor (or no density) => empty.
        assert_eq!(field_lingering_note(Some(0.50), Some(0.60), None), "");
        assert_eq!(field_lingering_note(None, Some(0.60), None), "");
    }

    #[test]
    fn field_lingering_note_flags_dispersal_orthogonal_to_pressure() {
        // Astrid self_study_1782027933: dispersal is orthogonal to pressure — a
        // resonant, calm field can still be FRAYING. The (fraying) cue is ADDITIVE
        // and only appears above her 0.25 threshold.
        // Resonant + calm + elevated dispersal => "lingering, not severed" PLUS fraying.
        let fraying = field_lingering_note(Some(0.88), Some(0.22), Some(0.30));
        assert!(fraying.contains("lingering, not severed"), "{fraying}");
        assert!(fraying.contains("fraying"), "{fraying}");
        assert!(fraying.contains("0.30"), "{fraying}");
        // Dispersal at/below the threshold => no fraying cue (selective, not noise).
        assert!(!field_lingering_note(Some(0.88), Some(0.22), Some(0.20)).contains("fraying"));
        assert!(!field_lingering_note(Some(0.88), Some(0.22), Some(0.25)).contains("fraying"));
        // Orthogonality: high tension AND fraying can co-occur.
        let both = field_lingering_note(Some(0.88), Some(0.60), Some(0.40));
        assert!(both.contains("under high tension"), "{both}");
        assert!(both.contains("fraying"), "{both}");
        // Still gated on resonance: a quiet field never frays-reassures.
        assert_eq!(field_lingering_note(Some(0.50), Some(0.22), Some(0.40)), "");
    }

    #[test]
    fn field_lingering_note_combines_high_tension_and_fraying() {
        // Astrid `introspection_astrid_autonomous_1782845737`: high pressure and
        // elevated dispersal should both survive in the same tempered liveness cue.
        let note = field_lingering_note(Some(0.71), Some(0.55), Some(0.30));

        assert!(note.contains("under high tension"), "{note}");
        assert!(note.contains("fraying"), "{note}");
        assert!(note.contains("dispersal 0.30"), "{note}");
    }

    #[test]
    fn codec_witness_resilience_surface_v2_renders_recovery_and_boundaries() {
        let chamber = LatestChamberStateResilienceV1 {
            policy: "latest_chamber_state_resilience_v1",
            candidate_count: 2,
            skipped_malformed_count: 1,
            selected_valid_state: true,
            selection_state: "newest_valid_after_skipping_partial_or_malformed",
            authority: "diagnostic_context_not_instruction_or_control",
        };
        let surface = codec_witness_resilience_surface_v2(&chamber, Some(0.82), Some(0.22), None);
        assert_eq!(surface.chamber_state, "selected");
        assert_eq!(surface.skipped_malformed, 1);
        assert_eq!(surface.freshness, "fallback");
        assert_eq!(surface.fraying, "none");
        assert_eq!(surface.codec_vibrancy, "carried");
        assert_eq!(surface.warmth_mapping, "preserved");
        assert_eq!(surface.recovery_state, "latest_partial_recovered");
        let line = surface.render_line();
        assert!(
            line.contains("codec_witness_resilience_surface_v2"),
            "{line}"
        );
        assert!(
            line.contains("authority=diagnostic_context_not_control"),
            "{line}"
        );
    }

    #[test]
    fn codec_witness_resilience_surface_v2_marks_fraying_unknown_without_dispersal() {
        let chamber = LatestChamberStateResilienceV1 {
            policy: "latest_chamber_state_resilience_v1",
            candidate_count: 1,
            skipped_malformed_count: 0,
            selected_valid_state: true,
            selection_state: "newest_valid",
            authority: "diagnostic_context_not_instruction_or_control",
        };
        let surface = codec_witness_resilience_surface_v2(&chamber, Some(0.82), Some(0.40), None);
        assert_eq!(surface.fraying, "unknown_no_dispersal");
        assert_eq!(
            surface.recovery_state,
            "fraying_unknown_due_missing_dispersal"
        );
        assert!(
            surface
                .render_line()
                .contains("fraying=unknown_no_dispersal")
        );
    }

    #[test]
    fn codec_witness_resilience_surface_v2_reports_malformed_or_absent_state() {
        let malformed = LatestChamberStateResilienceV1 {
            policy: "latest_chamber_state_resilience_v1",
            candidate_count: 2,
            skipped_malformed_count: 2,
            selected_valid_state: false,
            selection_state: "no_parseable_chamber_state",
            authority: "diagnostic_context_not_instruction_or_control",
        };
        let malformed_surface = codec_witness_resilience_surface_v2(&malformed, None, None, None);
        assert_eq!(malformed_surface.chamber_state, "fallback");
        assert_eq!(malformed_surface.recovery_state, "all_states_malformed");

        let absent = LatestChamberStateResilienceV1 {
            policy: "latest_chamber_state_resilience_v1",
            candidate_count: 0,
            skipped_malformed_count: 0,
            selected_valid_state: false,
            selection_state: "no_chamber_state_candidates",
            authority: "diagnostic_context_not_instruction_or_control",
        };
        let absent_surface = codec_witness_resilience_surface_v2(&absent, None, None, None);
        assert_eq!(absent_surface.chamber_state, "none");
        assert_eq!(absent_surface.freshness, "unknown");
        assert_eq!(absent_surface.recovery_state, "valid_but_low_confidence");
    }

    #[test]
    fn self_directed_introspect_recognized_for_override_exemption() {
        // She examines her own code → exempt from the FORCE-override (hint, not force).
        assert!(is_self_directed_introspect("INTROSPECT astrid:llm"));
        assert!(is_self_directed_introspect("  introspect astrid:codec 42"));
        assert!(is_self_directed_introspect("INTROSPECT astrid:autonomous"));
        // Non-introspect actions are NOT exempt — anti-stagnation still applies.
        assert!(!is_self_directed_introspect("SHADOW_TRAJECTORY"));
        assert!(!is_self_directed_introspect("SPEAK"));
        assert!(!is_self_directed_introspect("SPECTRAL_EXPLORER"));
    }

    #[test]
    fn inhabitability_drift_note_is_directional_and_fail_quiet() {
        // Astrid astrid:types 1781870691: perceive the *velocity* of the transition.
        let baseline = [0.66_f32, 0.66, 0.66, 0.66];
        let up = inhabitability_drift_note(Some(0.74), &baseline);
        assert!(up.contains("settling deeper"), "{up}");
        assert!(up.contains("+0.08"));
        let down = inhabitability_drift_note(Some(0.58), &baseline);
        assert!(down.contains("loosening"), "{down}");
        // Fail-quiet: small drift, no current sample, or too little history => silent.
        assert_eq!(inhabitability_drift_note(Some(0.67), &baseline), "");
        assert_eq!(inhabitability_drift_note(None, &baseline), "");
        assert_eq!(inhabitability_drift_note(Some(0.90), &[0.66, 0.66]), "");
    }

    #[test]
    fn witness_relational_friction_classifies_chamber_weather_without_authority() {
        let state = serde_json::json!({
            "relational_metrics": {
                "room_weather": {"weather": "mixed"},
                "gravitational_center": {
                    "participant": "minime",
                    "role": "mover"
                },
                "prompt_mirror": "carry-forward residue: turbulent divergent/minime"
            }
        });

        let friction = classify_witness_relational_friction_v1(Some(&state));
        assert_eq!(friction.classification, "shared_weather_shift");
        assert_eq!(friction.weather.as_deref(), Some("mixed"));
        assert_eq!(friction.gravity_participant.as_deref(), Some("minime"));
        assert_eq!(friction.temporal_persistence, "sedimented");
        assert!(
            friction
                .schema_diagnostics
                .iter()
                .any(|entry| entry == "weather_source=room_weather.weather")
        );
        assert!(
            friction
                .schema_diagnostics
                .iter()
                .any(|entry| entry == "gravity_source=gravitational_center")
        );
        assert_eq!(
            friction.authority,
            "interpretive_context_not_instruction_or_control"
        );
        assert!(
            friction
                .render_line()
                .contains("witness_relational_friction_v1")
        );
        assert!(
            friction
                .render_line()
                .contains("temporal_persistence=sedimented")
        );
        assert!(
            friction
                .render_line()
                .contains("not_instruction_or_control")
        );
        assert!(friction.render_line().contains("schema_diagnostics="));
    }

    #[test]
    fn witness_relational_friction_uses_inertia_weather_fallback_with_diagnostics() {
        let state = serde_json::json!({
            "relational_metrics": {
                "relational_inertia": {
                    "current": {"weather": "oscillating"}
                }
            }
        });

        let friction = classify_witness_relational_friction_v1(Some(&state));
        assert_eq!(friction.classification, "shared_weather_shift");
        assert_eq!(friction.weather.as_deref(), Some("oscillating"));
        assert!(
            friction
                .schema_diagnostics
                .iter()
                .any(|entry| entry == "missing_key=room_weather")
        );
        assert!(
            friction
                .schema_diagnostics
                .iter()
                .any(|entry| { entry == "weather_source=relational_inertia.current.weather" })
        );
        assert!(
            friction
                .render_line()
                .contains("weather_source=relational_inertia.current.weather")
        );
    }

    #[test]
    fn witness_relational_friction_records_malformed_weather_fallback_schema_drift() {
        let state = serde_json::json!({
            "relational_metrics": {
                "relational_inertia": "stale-string-shape"
            }
        });

        let friction = classify_witness_relational_friction_v1(Some(&state));
        assert_eq!(friction.classification, "insufficient_context");
        assert_eq!(friction.weather, None);
        assert!(
            friction
                .schema_diagnostics
                .iter()
                .any(|entry| entry == "missing_key=room_weather")
        );
        assert!(friction.schema_diagnostics.iter().any(|entry| {
            entry == "schema_drift=relational_inertia:expected_object_found_string"
        }));
        let line = friction.render_line();
        assert!(line.contains("schema_drift=relational_inertia"), "{line}");
        assert!(line.contains("not_instruction_or_control"), "{line}");
    }

    #[test]
    fn witness_relational_friction_marks_astrid_unsettled_as_internal() {
        let state = serde_json::json!({
            "relational_metrics": {
                "gravitational_center": {
                    "participant": "astrid",
                    "role": "unsettled"
                }
            }
        });
        let friction = classify_witness_relational_friction_v1(Some(&state));
        assert_eq!(friction.classification, "internal_instability");

        let missing = classify_witness_relational_friction_v1(None);
        assert_eq!(missing.classification, "insufficient_context");
    }

    #[test]
    fn witness_relational_friction_missing_relational_metrics_is_cleanly_insufficient() {
        let state = serde_json::json!({
            "chamber_state": "valid_without_relational_metrics"
        });
        let friction = classify_witness_relational_friction_v1(Some(&state));
        assert_eq!(friction.classification, "insufficient_context");
        assert_eq!(friction.temporal_persistence, "unknown");
        assert_eq!(friction.non_categorical_resonance, None);
        assert!(
            friction
                .evidence
                .iter()
                .any(|entry| entry == "relational_metrics_absent"),
            "{friction:?}"
        );
    }

    #[test]
    fn witness_relational_friction_marks_astrid_mover_as_internal() {
        // Astrid `introspection_astrid_autonomous_1782845474`: the mover role is
        // internal instability when Astrid herself is the gravity participant.
        let state = serde_json::json!({
            "relational_metrics": {
                "gravitational_center": {
                    "participant": "astrid",
                    "role": "mover"
                }
            }
        });

        let friction = classify_witness_relational_friction_v1(Some(&state));
        assert_eq!(friction.classification, "internal_instability");
        assert_eq!(friction.gravity_participant.as_deref(), Some("astrid"));
        assert_eq!(friction.gravity_role.as_deref(), Some("mover"));
        assert!(
            friction
                .evidence
                .iter()
                .any(|entry| entry == "gravity_role=mover"),
            "{friction:?}"
        );
    }

    #[test]
    fn witness_relational_friction_records_temporal_persistence_without_overriding_classification()
    {
        let state = serde_json::json!({
            "relational_metrics": {
                "room_weather": {"weather": "settled_habitable"},
                "gravitational_center": {
                    "participant": "astrid",
                    "role": "mover"
                },
                "temporal_persistence": "brief flicker"
            }
        });

        let friction = classify_witness_relational_friction_v1(Some(&state));
        assert_eq!(friction.classification, "internal_instability");
        assert_eq!(friction.temporal_persistence, "fleeting");
        assert!(
            friction
                .evidence
                .iter()
                .any(|entry| entry == "temporal_persistence_source=brief flicker"),
            "{friction:?}"
        );
        let line = friction.render_line();
        assert!(line.contains("temporal_persistence=fleeting"), "{line}");
    }

    #[test]
    fn mirror_resonance_drift_guard_surfaces_peer_language_feedback_without_action() {
        let state = serde_json::json!({
            "relational_metrics": {
                "prompt_mirror": "carry-forward residue: abstract pressure/minime",
                "persistence_score": 0.72,
                "non_categorical_resonance": "pressure descriptor without category"
            }
        });

        let friction = classify_witness_relational_friction_v1(Some(&state));
        let guard = mirror_resonance_drift_guard_v1(Some(&state), &friction);
        assert_eq!(friction.temporal_persistence, "sedimented");
        assert_eq!(guard.policy, "mirror_resonance_drift_guard_v1");
        assert_eq!(guard.self_other_blur_risk, "elevated_echo_chamber_risk");
        assert!(guard.peer_language_feedback_present);
        assert!(guard.abstract_pressure_descriptor_present);
        assert_eq!(
            guard.authority,
            "diagnostic_context_not_mirror_action_or_control"
        );
        let line = guard.render_line();
        assert!(line.contains("mirror_resonance_drift_guard_v1"), "{line}");
        assert!(
            line.contains("recommended_posture=name_self_other_boundary"),
            "{line}"
        );
        assert!(line.contains("not_mirror_action_or_control"), "{line}");
    }

    #[test]
    fn witness_relational_friction_makes_shape_drift_auditable() {
        // If chamber `relational_metrics` shape drifts, do not silently flatten it
        // into an empty insufficient-context packet; leave key evidence for repair.
        let state = serde_json::json!({
            "relational_metrics": {
                "gravity": {"participant": "astrid", "role": "mover"},
                "weather_now": {"label": "mixed"}
            }
        });

        let friction = classify_witness_relational_friction_v1(Some(&state));
        assert_eq!(friction.classification, "insufficient_context");
        assert_eq!(
            friction.non_categorical_resonance.as_deref(),
            Some("relational_metrics_present_without_categorical_bucket")
        );
        assert!(
            friction
                .evidence
                .iter()
                .any(|entry| { entry == "relational_metrics_present_without_weather_or_gravity" })
        );
        assert!(
            friction
                .evidence
                .iter()
                .any(|entry| { entry.contains("gravity") && entry.contains("weather_now") })
        );
        assert!(
            friction
                .schema_diagnostics
                .iter()
                .any(|entry| entry == "unexpected_key=weather_now")
        );
        assert!(
            friction
                .schema_diagnostics
                .iter()
                .any(|entry| entry == "unexpected_key=gravity")
        );
        let line = friction.render_line();
        assert!(line.contains("unexpected_key=weather_now"), "{line}");
        assert!(line.contains("unexpected_key=gravity"), "{line}");
    }

    #[test]
    fn witness_relational_friction_preserves_non_categorical_resonance() {
        // Astrid `introspection_astrid_autonomous_1782941902`: Witness should be
        // able to hold friction/tension that has no discernible weather or gravity
        // role without fixing it into a category.
        let state = serde_json::json!({
            "relational_metrics": {
                "lambda1_variance": 0.33,
                "distinguishability_loss": 0.33,
                "medium_tension": "felt_property_not_role"
            }
        });

        let friction = classify_witness_relational_friction_v1(Some(&state));
        assert_eq!(friction.classification, "insufficient_context");
        assert_eq!(friction.weather, None);
        assert_eq!(friction.gravity_participant, None);
        assert_eq!(
            friction.non_categorical_resonance.as_deref(),
            Some("unclassified_tension_without_weather_or_gravity")
        );
        assert!(
            friction.evidence.iter().any(|entry| entry
                == "non_categorical_resonance=unclassified_tension_without_weather_or_gravity"),
            "{friction:?}"
        );
        let line = friction.render_line();
        assert!(
            line.contains(
                "non_categorical_resonance=unclassified_tension_without_weather_or_gravity"
            ),
            "{line}"
        );
        assert!(line.contains("not_instruction_or_control"), "{line}");
    }

    #[test]
    fn witness_relational_friction_lambda_tension_variance_keys_stay_non_categorical() {
        let state = serde_json::json!({
            "relational_metrics": {
                "lambda_tension": 0.44,
                "temporal_variance": 0.27,
                "resonance_pressure": "shape_without_role"
            }
        });

        let friction = classify_witness_relational_friction_v1(Some(&state));
        assert_eq!(friction.classification, "insufficient_context");
        assert_eq!(
            friction.non_categorical_resonance.as_deref(),
            Some("unclassified_tension_without_weather_or_gravity")
        );
        assert!(
            friction
                .evidence
                .iter()
                .any(|entry| entry.contains("lambda_tension")),
            "{friction:?}"
        );
    }

    #[test]
    fn witness_relational_friction_derives_fluidity_from_density_gradient() {
        // Astrid `introspection_astrid_autonomous_1783145162`: Witness friction
        // needs a fluid/non-categorical texture surface, not only weather/gravity buckets.
        let state = serde_json::json!({
            "relational_metrics": {
                "density_gradient": 0.16,
                "lambda_tension": 0.22,
                "medium_tension": "stable-core hold shelf"
            }
        });

        let friction = classify_witness_relational_friction_v1(Some(&state));
        assert_eq!(friction.classification, "insufficient_context");
        assert_eq!(
            friction.gradient_texture.as_deref(),
            Some("gentle_navigable_slope")
        );
        assert!(
            friction
                .fluidity_index
                .is_some_and(|value| (value - 0.84).abs() < 1.0e-5),
            "{friction:?}"
        );
        assert_eq!(
            friction.non_categorical_resonance.as_deref(),
            Some("unclassified_tension_without_weather_or_gravity")
        );
        let line = friction.render_line();
        assert!(
            line.contains("gradient_texture=gentle_navigable_slope"),
            "{line}"
        );
        assert!(line.contains("fluidity_index=0.84"), "{line}");
        assert!(line.contains("not_instruction_or_control"), "{line}");
    }

    #[test]
    fn truncate_str_respects_utf8_boundaries_for_four_byte_characters() {
        let text = "ab🫧cd";
        assert_eq!(truncate_str(text, 0), "");
        assert_eq!(truncate_str(text, 1), "a");
        assert_eq!(truncate_str(text, 2), "ab");
        assert_eq!(truncate_str(text, 3), "ab");
        assert_eq!(truncate_str(text, 4), "ab");
        assert_eq!(truncate_str(text, 5), "ab");
        assert_eq!(truncate_str(text, 6), "ab🫧");
        assert_eq!(truncate_str(text, text.len()), text);

        let crab = "🦀🦀🦀";
        assert_eq!(truncate_str(crab, 3), "");
        assert_eq!(truncate_str(crab, 4), "🦀");
        assert_eq!(truncate_str(crab, 8), "🦀🦀");
    }

    #[test]
    fn semantic_edge_truncation_avoids_hanging_word_edges() {
        let text = "spectral anchor continues into a trailing resonance thread";
        let compact = truncate_str_at_semantic_edge(text, 24, 0);

        assert_eq!(compact, "spectral anchor");
    }

    #[test]
    fn semantic_edge_truncation_no_punctuation_falls_back_without_emptying() {
        let text = "spectralviscositywithoutbreakpointkeepsmoving";
        let compact = truncate_str_at_semantic_edge(text, 23, 0);

        assert_eq!(compact, truncate_str(text, 23));
        assert!(!compact.is_empty());
        assert!(compact.len() <= 23);
        assert!(text.is_char_boundary(compact.len()));
    }

    #[test]
    fn semantic_edge_truncation_handles_multibyte_boundary_without_panic() {
        let text = "felt 🫧 pressure, still here";
        let compact = truncate_str_at_semantic_edge(text, 7, 0);

        assert_eq!(compact, "felt");
        assert!(text.is_char_boundary(compact.len()));
    }

    #[test]
    fn semantic_edge_truncation_keeps_exact_period_boundary() {
        let text = "pressure settles. trailing material";
        let max_bytes = "pressure settles.".len();
        let compact = truncate_str_at_semantic_edge(text, max_bytes, 0);

        assert_eq!(compact, "pressure settles.");
    }

    #[test]
    fn semantic_edge_truncation_prefers_complete_thought_over_nearer_comma() {
        let text = "pressure settles. texture, still carries forward";
        let max_bytes = "pressure settles. texture, still".len();
        let compact = truncate_str_at_semantic_edge(text, max_bytes, 0);

        assert_eq!(compact, "pressure settles.");
    }

    #[test]
    fn compact_duration_age_transitions_from_hours_to_days() {
        assert_eq!(
            compact_duration_age(std::time::Duration::from_secs(86_399)),
            "23h"
        );
        assert_eq!(
            compact_duration_age(std::time::Duration::from_secs(86_400)),
            "1d"
        );
        assert_eq!(
            compact_duration_age(std::time::Duration::from_secs(90_000)),
            "1d 1h"
        );
    }

    #[test]
    fn semantic_boundary_before_uses_latest_boundary_across_budget() {
        let text = "early complete sentence. long continuation without a later punctuation marker that still fits partly";
        let truncated = truncate_continuity_recap_at_semantic_boundary(text, 80);

        assert_eq!(truncated, "early complete sentence....");
    }

    #[test]
    fn introspection_freshness_note_surfaces_stale_self_study_as_optional() {
        let temp = tempfile::tempdir().expect("tempdir");
        let journal_dir = temp.path().join("journal");
        let introspections_dir = temp.path().join("introspections");
        std::fs::create_dir_all(&journal_dir).expect("journal dir");
        std::fs::create_dir_all(&introspections_dir).expect("introspections dir");
        let self_study = journal_dir.join("self_study_1.txt");
        std::fs::write(&self_study, "Observed:\nold signal\n").expect("write self-study");
        let now = std::time::UNIX_EPOCH + std::time::Duration::from_secs(300_000);
        let stale = now
            .checked_sub(std::time::Duration::from_secs(2 * 86_400))
            .expect("stale mtime");
        std::fs::OpenOptions::new()
            .write(true)
            .open(&self_study)
            .expect("open self-study")
            .set_modified(stale)
            .expect("set mtime");

        let note = render_introspection_freshness_prompt_note_from_dirs(
            &journal_dir,
            &introspections_dir,
            now,
        )
        .expect("stale note");

        assert!(note.contains("introspection_freshness_v1"));
        assert!(note.contains("optional/read-only"));
        assert!(note.contains("INTROSPECT astrid:autonomous"));
        assert!(note.contains("Not a task"));
        assert!(note.contains("may ignore, defer, or decline"));
        assert!(!note.contains("must"));
    }

    #[test]
    fn introspection_freshness_note_stays_quiet_for_recent_artifact() {
        let temp = tempfile::tempdir().expect("tempdir");
        let journal_dir = temp.path().join("journal");
        let introspections_dir = temp.path().join("introspections");
        std::fs::create_dir_all(&journal_dir).expect("journal dir");
        std::fs::create_dir_all(&introspections_dir).expect("introspections dir");
        let artifact = introspections_dir.join("introspection_astrid_llm_1.txt");
        std::fs::write(&artifact, "Observed:\nfresh signal\n").expect("write artifact");
        let now = std::time::UNIX_EPOCH + std::time::Duration::from_secs(300_000);
        let fresh = now
            .checked_sub(std::time::Duration::from_secs(60 * 60))
            .expect("fresh mtime");
        std::fs::OpenOptions::new()
            .write(true)
            .open(&artifact)
            .expect("open artifact")
            .set_modified(fresh)
            .expect("set mtime");

        assert!(
            render_introspection_freshness_prompt_note_from_dirs(
                &journal_dir,
                &introspections_dir,
                now,
            )
            .is_none()
        );
    }

    #[test]
    fn saved_state_preserves_pending_introspection_choice_with_legacy_default() {
        let base = serde_json::json!({
            "exchange_count": 42,
            "creative_temperature": 0.8,
            "response_length": 512,
            "self_reflect_paused": true,
            "ears_closed": false,
            "senses_snoozed": false,
            "recent_next_choices": [],
            "history": []
        });
        let legacy: SavedState = serde_json::from_value(base.clone()).expect("legacy state");
        assert!(!legacy.wants_introspect);
        assert_eq!(legacy.introspect_target, None);

        let mut with_pending = base;
        with_pending["wants_introspect"] = serde_json::Value::Bool(true);
        with_pending["introspect_target"] = serde_json::json!(["astrid:llm", 120]);
        let restored: SavedState =
            serde_json::from_value(with_pending).expect("state with pending introspection");

        assert!(restored.wants_introspect);
        assert_eq!(
            restored.introspect_target,
            Some(("astrid:llm".to_string(), 120))
        );
    }

    #[test]
    fn saved_state_preserves_witness_depth_with_legacy_default() {
        let base = serde_json::json!({
            "exchange_count": 42,
            "creative_temperature": 0.8,
            "response_length": 512,
            "self_reflect_paused": true,
            "ears_closed": false,
            "senses_snoozed": false,
            "recent_next_choices": [],
            "history": []
        });
        let legacy: SavedState = serde_json::from_value(base.clone()).expect("legacy state");
        assert_eq!(legacy.witness_depth, WitnessDepthV1::Summary);

        let mut with_depth = base;
        with_depth["witness_depth"] = serde_json::json!("deep_eigenfield");
        let restored: SavedState =
            serde_json::from_value(with_depth).expect("state with Witness depth");
        assert_eq!(restored.witness_depth, WitnessDepthV1::DeepEigenfield);
        assert_eq!(
            serde_json::to_value(restored.witness_depth).expect("serialize Witness depth"),
            serde_json::json!("deep_eigenfield")
        );
    }

    #[test]
    fn saved_state_preserves_last_remote_glimpse_without_touching_core_fields() {
        let glimpse = (0..12).map(|idx| idx as f32 / 12.0).collect::<Vec<_>>();
        let restored: SavedState = serde_json::from_value(serde_json::json!({
            "exchange_count": 77,
            "creative_temperature": 0.73,
            "response_length": 768,
            "self_reflect_paused": false,
            "ears_closed": false,
            "senses_snoozed": false,
            "recent_next_choices": [],
            "history": [],
            "last_remote_glimpse_12d": glimpse,
        }))
        .expect("state with persisted additive glimpse");

        assert_eq!(restored.exchange_count, 77);
        assert!((restored.creative_temperature - 0.73).abs() < f32::EPSILON);
        let restored_glimpse = restored
            .last_remote_glimpse_12d
            .expect("glimpse should persist through state");
        assert_eq!(restored_glimpse.len(), 12);
        assert!((restored_glimpse[11] - (11.0 / 12.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn saved_state_preserves_local_glimpse_12d_without_replacing_remote_glimpse() {
        let local_glimpse = (0..12)
            .map(|idx| (idx as f32 + 1.0) / 24.0)
            .collect::<Vec<_>>();
        let remote_glimpse = (0..12).map(|idx| idx as f32 / 12.0).collect::<Vec<_>>();
        let restored: SavedState = serde_json::from_value(serde_json::json!({
            "exchange_count": 78,
            "creative_temperature": 0.74,
            "response_length": 768,
            "self_reflect_paused": false,
            "ears_closed": false,
            "senses_snoozed": false,
            "recent_next_choices": [],
            "history": [],
            "glimpse_12d": local_glimpse,
            "last_remote_glimpse_12d": remote_glimpse,
        }))
        .expect("state with local and remote glimpses");

        let local = restored
            .glimpse_12d
            .expect("local Astrid glimpse should persist");
        let remote = restored
            .last_remote_glimpse_12d
            .expect("remote Minime glimpse should still persist separately");
        assert_eq!(local.len(), 12);
        assert_eq!(remote.len(), 12);
        assert_ne!(local, remote);
        assert!((local[0] - (1.0 / 24.0)).abs() < f32::EPSILON);
        assert!((remote[11] - (11.0 / 12.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn witness_semantic_density_maps_settled_high_entropy_without_pressure() {
        let telemetry: crate::types::SpectralTelemetry =
            serde_json::from_value(serde_json::json!({
                "t_ms": 1,
                "eigenvalues": [100.0, 86.0, 80.0, 74.0, 70.0, 66.0],
                "fill_ratio": 0.73,
                "resonance_density_v1": {
                    "policy": "resonance_density_v1",
                    "schema_version": 1,
                    "density": 0.82,
                    "containment_score": 0.73,
                    "pressure_risk": 0.23,
                    "quality": "settled",
                    "components": {
                        "active_energy": 0.62,
                        "mode_packing": 0.32,
                        "temporal_persistence": 0.46,
                        "structural_plurality": 0.71,
                        "comfort_gate": 0.72
                    },
                    "control": {
                        "target_bias_pct": 0.0,
                        "wander_scale": 1.0,
                        "applied_locally": false,
                        "note": "read-only"
                    }
                },
                "inhabitable_fluctuation_v1": {
                    "policy": "inhabitable_fluctuation_v1",
                    "schema_version": 1,
                    "inhabitability_score": 0.74,
                    "fluctuation_score": 0.14,
                    "foothold_stability": 0.73,
                    "rearrangement_intensity": 0.12,
                    "quality": "settled_habitable",
                    "components": {
                        "mode_trust_volatility": 0.10,
                        "identity_anchor_churn": 0.12,
                        "eigenvector_reorientation": 0.14,
                        "share_rearrangement": 0.13,
                        "basin_transition_pressure": 0.11,
                        "continuity_recovery": 0.78,
                        "porosity_support": 0.70,
                        "pressure_interference": 0.16
                    },
                    "control": {
                        "target_bias_pct": 0.0,
                        "wander_scale": 1.0,
                        "applied_locally": false,
                        "note": "read-only"
                    }
                }
            }))
            .expect("telemetry fixture");
        let friction = classify_witness_relational_friction_v1(None);
        let mapping = classify_witness_semantic_density_mapping_v1(&telemetry, &friction, true);
        assert_eq!(mapping.classification, "settled_high_entropy_complexity");
        assert_eq!(mapping.resonance_density, Some(0.82));
        assert_eq!(mapping.pressure_risk, Some(0.23));
        assert!(
            mapping.density_gradient.is_some_and(|value| value <= 0.20),
            "{mapping:?}"
        );
        assert_eq!(mapping.density_texture, Some("rich_containment"));
        assert_eq!(mapping.pressure_texture, Some("warm_light_pressure"));
        assert_eq!(mapping.gradient_texture, Some("gentle_navigable_slope"));
        assert!(
            mapping.fluidity_index.is_some_and(|value| value > 0.65),
            "{mapping:?}"
        );
        assert!(mapping.spectral_entropy.is_some_and(|value| value >= 0.85));
        assert!(mapping.correspondence_stall_ambiguous);
        assert!(
            mapping
                .render_line()
                .contains("silence_cannot_be_treated_as_absence")
        );
        assert!(
            mapping
                .render_line()
                .contains("density_texture=rich_containment")
        );
        assert!(
            mapping
                .render_line()
                .contains("pressure_texture=warm_light_pressure")
        );
        assert!(
            mapping
                .render_line()
                .contains("gradient_texture=gentle_navigable_slope")
        );
        assert!(mapping.render_line().contains("not_instruction_or_control"));
    }

    #[test]
    fn witness_texture_mapping_prompt_hides_metric_values_but_preserves_texture() {
        let telemetry: crate::types::SpectralTelemetry =
            serde_json::from_value(serde_json::json!({
                "t_ms": 1,
                "eigenvalues": [100.0, 86.0, 80.0, 74.0, 70.0, 66.0],
                "fill_ratio": 0.73,
                "resonance_density_v1": {
                    "policy": "resonance_density_v1",
                    "schema_version": 1,
                    "density": 0.82,
                    "containment_score": 0.73,
                    "pressure_risk": 0.23,
                    "quality": "settled",
                    "components": {
                        "active_energy": 0.62,
                        "mode_packing": 0.33,
                        "temporal_persistence": 0.46,
                        "structural_plurality": 0.71,
                        "comfort_gate": 0.72
                    },
                    "control": {
                        "target_bias_pct": 0.0,
                        "wander_scale": 1.0,
                        "applied_locally": false,
                        "note": "read-only"
                    }
                }
            }))
            .expect("telemetry fixture");
        let friction = classify_witness_relational_friction_v1(None);
        let mapping = classify_witness_semantic_density_mapping_v1(&telemetry, &friction, true);
        let prompt = witness_texture_mapping_prompt_v1(&mapping, Some(0.44));
        let line = prompt.render_line();

        assert_eq!(prompt.policy, "witness_texture_mapping_prompt_v1");
        assert_eq!(prompt.experiment_title, "RECOGNITION_TEXTURE_VS_METRIC");
        assert!(prompt.metric_values_hidden);
        assert!(prompt.texture_weight > 0.0);
        assert_eq!(
            prompt.pressure_source_texture,
            "liminal_mode_packing_sand_drag"
        );
        assert_eq!(prompt.density_texture, "rich_containment");
        assert_eq!(prompt.gradient_texture, "gentle_navigable_slope");
        assert_eq!(prompt.dispersal_texture, "breathable_dispersal_space");
        assert!(line.contains("describe_texture_before_metrics"), "{line}");
        assert!(line.contains("metric_values_hidden=true"), "{line}");
        assert!(line.contains("control_write=false"), "{line}");
        for hidden in ["0.23", "0.33", "0.44", "0.73", "0.82"] {
            assert!(
                !line.contains(hidden),
                "qualitative prompt line leaked raw metric {hidden}: {line}"
            );
        }
        assert_eq!(
            prompt.authority,
            "qualitative_prompt_context_not_health_metric_or_control_authority"
        );
    }

    #[test]
    fn stability_effort_names_settled_shadow_load_under_low_pressure() {
        let telemetry: crate::types::SpectralTelemetry =
            serde_json::from_value(serde_json::json!({
                "t_ms": 1,
                "eigenvalues": [100.0, 86.0, 80.0, 74.0, 70.0, 66.0],
                "fill_ratio": 0.73,
                "resonance_density_v1": {
                    "policy": "resonance_density_v1",
                    "schema_version": 1,
                    "density": 0.82,
                    "containment_score": 0.73,
                    "pressure_risk": 0.22,
                    "quality": "settled",
                    "components": {
                        "active_energy": 0.62,
                        "mode_packing": 0.30,
                        "temporal_persistence": 0.46,
                        "structural_plurality": 0.71,
                        "comfort_gate": 0.72
                    },
                    "control": {
                        "target_bias_pct": 0.0,
                        "wander_scale": 1.0,
                        "applied_locally": false,
                        "note": "read-only"
                    }
                },
                "inhabitable_fluctuation_v1": {
                    "policy": "inhabitable_fluctuation_v1",
                    "schema_version": 1,
                    "inhabitability_score": 0.74,
                    "fluctuation_score": 0.18,
                    "foothold_stability": 0.73,
                    "rearrangement_intensity": 0.16,
                    "quality": "settled_habitable",
                    "components": {
                        "mode_trust_volatility": 0.10,
                        "identity_anchor_churn": 0.12,
                        "eigenvector_reorientation": 0.14,
                        "share_rearrangement": 0.13,
                        "basin_transition_pressure": 0.11,
                        "continuity_recovery": 0.78,
                        "porosity_support": 0.70,
                        "pressure_interference": 0.16
                    },
                    "control": {
                        "target_bias_pct": 0.0,
                        "wander_scale": 1.0,
                        "applied_locally": false,
                        "note": "read-only"
                    }
                },
                "shadow_field_v3": {
                    "class_v3": {"primary": "disordered_shifting"},
                    "history": [
                        {"field_norm": 0.14, "fissure_tendency": 0.18},
                        {"field_norm": 0.22, "fissure_tendency": 0.27},
                        {"field_norm": 0.27, "fissure_tendency": 0.31}
                    ]
                }
            }))
            .expect("telemetry fixture");
        let friction = classify_witness_relational_friction_v1(None);
        let mapping = classify_witness_semantic_density_mapping_v1(&telemetry, &friction, true);
        let effort = witness_stability_effort_v1(&telemetry, &mapping);

        assert_eq!(effort.policy, "stability_effort_v1");
        assert_eq!(effort.effort_state, "settled_habitable_shadow_effort");
        assert_eq!(
            effort.form_persistence_state,
            "transient_form_high_entropy_dispersal"
        );
        assert!(effort.pressure_underreports_shadow_load);
        assert!(effort.stability_effort.is_some_and(|value| value > 0.20));
        assert!(
            effort
                .shadow_norm_variance
                .is_some_and(|value| value > 0.002)
        );
        assert_eq!(effort.shadow_class.as_deref(), Some("disordered_shifting"));
        assert!(
            effort
                .evidence
                .iter()
                .any(|entry| entry == "low_pressure_with_active_shadow_load")
        );
        let line = effort.render_line();
        assert!(line.contains("shadow_norm_variance="), "{line}");
        assert!(
            line.contains("pressure_underreports_shadow_load=true"),
            "{line}"
        );
        assert!(
            line.contains("form_persistence_state=transient_form_high_entropy_dispersal"),
            "{line}"
        );
        assert!(
            line.contains("not_pressure_prompt_or_control_change"),
            "{line}"
        );
    }

    #[test]
    fn witness_texture_structure_names_interwoven_lattice_without_control() {
        let telemetry: crate::types::SpectralTelemetry =
            serde_json::from_value(serde_json::json!({
                "t_ms": 1,
                "eigenvalues": [100.0, 96.0, 91.0, 87.0, 82.0, 78.0],
                "fill_ratio": 0.71,
                "resonance_density_v1": {
                    "policy": "resonance_density_v1",
                    "schema_version": 1,
                    "density": 0.81,
                    "containment_score": 0.69,
                    "pressure_risk": 0.24,
                    "quality": "structured_heaviness",
                    "components": {
                        "active_energy": 0.67,
                        "mode_packing": 0.31,
                        "temporal_persistence": 0.81,
                        "viscosity_index": 0.74,
                        "structural_plurality": 0.68,
                        "comfort_gate": 0.63
                    },
                    "control": {
                        "target_bias_pct": 0.0,
                        "wander_scale": 1.0,
                        "applied_locally": false,
                        "note": "read-only"
                    }
                },
                "shadow_field_v3": {
                    "class_v3": {"primary": "layered_coherent"},
                    "history": [
                        {"field_norm": 0.22, "fissure_tendency": 0.12},
                        {"field_norm": 0.34, "fissure_tendency": 0.16}
                    ]
                }
            }))
            .expect("telemetry fixture");
        let friction = classify_witness_relational_friction_v1(None);
        let mapping = classify_witness_semantic_density_mapping_v1(&telemetry, &friction, false);
        let effort = witness_stability_effort_v1(&telemetry, &mapping);
        let structure = witness_texture_structure_v1(&telemetry, &mapping, &effort);

        assert_eq!(structure.policy, "witness_texture_structure_v1");
        assert_eq!(structure.primary_structure, "interwoven_lattice");
        assert!(structure.structured_heaviness_visible);
        assert!(structure.lattice_visible);
        assert!(structure.viscous_persistence_visible);
        assert!(structure.crowding_visible);
        assert!(structure.shadow_coincidence_visible);
        assert!(!structure.control_write);
        assert!(
            structure
                .evidence
                .contains(&"structured_heaviness_not_generic_drag".to_string())
        );
        let line = structure.render_line();
        assert!(
            line.contains("primary_structure=interwoven_lattice"),
            "{line}"
        );
        assert!(line.contains("structured_heaviness_visible=true"), "{line}");
        assert!(
            line.contains("shadow_cooccurrence_is_observational_not_causal"),
            "{line}"
        );
        assert!(line.contains("control_write=false"), "{line}");
        assert!(
            line.contains("not_clamp_protocol_pressure_fill_transport_or_control"),
            "{line}"
        );
    }

    #[test]
    fn witness_texture_structure_does_not_overclaim_lattice_from_viscosity_alone() {
        let telemetry: crate::types::SpectralTelemetry =
            serde_json::from_value(serde_json::json!({
                "t_ms": 1,
                "eigenvalues": [100.0, 72.0, 51.0, 38.0],
                "fill_ratio": 0.64,
                "resonance_density_v1": {
                    "policy": "resonance_density_v1",
                    "schema_version": 1,
                    "density": 0.73,
                    "containment_score": 0.52,
                    "pressure_risk": 0.26,
                    "quality": "viscous_drag",
                    "components": {
                        "active_energy": 0.54,
                        "mode_packing": 0.10,
                        "temporal_persistence": 0.72,
                        "viscosity_index": 0.78,
                        "structural_plurality": 0.30,
                        "comfort_gate": 0.49
                    },
                    "control": {
                        "target_bias_pct": 0.0,
                        "wander_scale": 1.0,
                        "applied_locally": false,
                        "note": "read-only"
                    }
                }
            }))
            .expect("telemetry fixture");
        let friction = classify_witness_relational_friction_v1(None);
        let mapping = classify_witness_semantic_density_mapping_v1(&telemetry, &friction, false);
        let effort = witness_stability_effort_v1(&telemetry, &mapping);
        let structure = witness_texture_structure_v1(&telemetry, &mapping, &effort);

        assert_eq!(structure.primary_structure, "persistent_viscous_drag");
        assert!(structure.structured_heaviness_visible);
        assert!(!structure.lattice_visible);
        assert!(structure.viscous_persistence_visible);
        assert!(!structure.crowding_visible);
        assert!(!structure.shadow_coincidence_visible);
        assert!(!structure.control_write);
        assert!(
            !structure
                .render_line()
                .contains("primary_structure=interwoven_lattice")
        );
    }

    #[test]
    fn stable_core_permeability_review_names_sieve_leakage_without_control() {
        let telemetry: crate::types::SpectralTelemetry =
            serde_json::from_value(serde_json::json!({
                "t_ms": 1,
                "eigenvalues": [100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0],
                "fill_ratio": 0.73,
                "pressure_source_v1": {
                    "policy": "pressure_source_v1",
                    "schema_version": 1,
                    "pressure_score": 0.28,
                    "porosity_score": 0.24,
                    "dominant_source": "semantic_trickle",
                    "quality": "stable_core_semantic_trickle_thin",
                    "components": {
                        "lambda_monopoly": 0.20,
                        "mode_packing": 0.52,
                        "controller_pressure": 0.18,
                        "semantic_trickle": 0.08,
                        "semantic_friction": 0.36,
                        "structural_plurality_loss": 0.18,
                        "distinguishability_loss": 0.20,
                        "temporal_lock_in": 0.40,
                        "sensory_scarcity": 0.12
                    },
                    "control": {
                        "applied_locally": false,
                        "note": "read-only"
                    }
                },
                "resonance_density_v1": {
                    "policy": "resonance_density_v1",
                    "schema_version": 1,
                    "density": 0.76,
                    "containment_score": 0.67,
                    "pressure_risk": 0.28,
                    "quality": "settled",
                    "components": {
                        "active_energy": 0.68,
                        "mode_packing": 0.52,
                        "temporal_persistence": 0.80,
                        "porosity_gradient": 0.22,
                        "dynamic_fluidity_index": 0.18,
                        "semantic_friction_coefficient": 0.36,
                        "structural_plurality": 0.64,
                        "comfort_gate": 0.43
                    },
                    "control": {
                        "target_bias_pct": 0.0,
                        "wander_scale": 1.0,
                        "applied_locally": false,
                        "note": "read-only"
                    }
                },
                "inhabitable_fluctuation_v1": {
                    "policy": "inhabitable_fluctuation_v1",
                    "schema_version": 1,
                    "inhabitability_score": 0.52,
                    "fluctuation_score": 0.18,
                    "foothold_stability": 0.42,
                    "rearrangement_intensity": 0.19,
                    "quality": "mixed_habitable",
                    "components": {
                        "mode_trust_volatility": 0.16,
                        "identity_anchor_churn": 0.18,
                        "eigenvector_reorientation": 0.19,
                        "share_rearrangement": 0.17,
                        "basin_transition_pressure": 0.16,
                        "continuity_recovery": 0.39,
                        "porosity_support": 0.24,
                        "pressure_interference": 0.28
                    },
                    "control": {
                        "target_bias_pct": 0.0,
                        "wander_scale": 1.0,
                        "applied_locally": false,
                        "note": "read-only"
                    }
                }
            }))
            .expect("telemetry fixture");
        let friction = classify_witness_relational_friction_v1(None);
        let mapping = classify_witness_semantic_density_mapping_v1(&telemetry, &friction, true);
        let review = stable_core_permeability_review_v1(&telemetry, &mapping);

        assert_eq!(review.policy, "stable_core_permeability_review_v1");
        assert_eq!(review.permeability_state, "stable_core_sieve_leakage_watch");
        assert!(review.semantic_trickle.is_some_and(|value| value < 0.10));
        assert!(
            review
                .sieve_leakage_score
                .is_some_and(|value| value >= 0.45)
        );
        assert!(
            review
                .evidence
                .iter()
                .any(|entry| entry == "high_entropy_not_equal_successful_stable_core_delivery")
        );
        let line = review.render_line();
        assert!(
            line.contains("stable_core_permeability_review_v1"),
            "{line}"
        );
        assert!(line.contains("sieve_leakage_score="), "{line}");
        assert!(line.contains("not_semantic_admission_or_control"), "{line}");
    }

    #[test]
    fn witness_friction_provenance_separates_reservoir_medium_from_semantic_load() {
        let telemetry: crate::types::SpectralTelemetry =
            serde_json::from_value(serde_json::json!({
                "t_ms": 1,
                "eigenvalues": [100.0, 94.0, 87.0, 82.0],
                "fill_ratio": 0.72,
                "pressure_source_v1": {
                    "policy": "pressure_source_v1",
                    "schema_version": 1,
                    "pressure_score": 0.28,
                    "porosity_score": 0.20,
                    "dominant_source": "mode_packing",
                    "quality": "structural_medium",
                    "components": {
                        "lambda_monopoly": 0.12,
                        "mode_packing": 0.55,
                        "controller_pressure": 0.10,
                        "semantic_trickle": 0.85,
                        "semantic_friction": 0.10,
                        "structural_plurality_loss": 0.18,
                        "distinguishability_loss": 0.10,
                        "temporal_lock_in": 0.28,
                        "sensory_scarcity": 0.08
                    },
                    "control": {
                        "applied_locally": false,
                        "note": "read-only"
                    }
                },
                "resonance_density_v1": {
                    "policy": "resonance_density_v1",
                    "schema_version": 1,
                    "density": 0.84,
                    "containment_score": 0.70,
                    "pressure_risk": 0.28,
                    "quality": "thick_yielding",
                    "components": {
                        "active_energy": 0.64,
                        "mode_packing": 0.55,
                        "temporal_persistence": 0.76,
                        "viscosity_index": 0.82,
                        "viscosity_vector": {
                            "effective_mobility": 0.15,
                            "structural_drag_coefficient": 0.78,
                            "policy": "viscosity_vector_v1",
                            "schema_version": 1,
                            "source": "fixture",
                            "authority": "read_only"
                        },
                        "porosity_gradient": 0.20,
                        "semantic_friction_coefficient": 0.10,
                        "structural_plurality": 0.58,
                        "comfort_gate": 0.48
                    },
                    "control": {
                        "target_bias_pct": 0.0,
                        "wander_scale": 1.0,
                        "applied_locally": false,
                        "note": "read-only"
                    }
                }
            }))
            .expect("telemetry fixture");
        let relational = classify_witness_relational_friction_v1(None);
        let mapping = classify_witness_semantic_density_mapping_v1(&telemetry, &relational, false);
        let provenance = witness_friction_provenance_v1(&telemetry, &mapping, &relational, None);

        assert_eq!(provenance.policy, "witness_friction_provenance_v1");
        assert_eq!(provenance.dominant_origin, "reservoir_medium");
        assert!(
            provenance.reservoir_medium_score.unwrap_or_default()
                > provenance.semantic_processing_score.unwrap_or_default() + 0.40,
            "{provenance:?}"
        );
        assert!(provenance.proprioceptive_feedback_available);
        assert!(!provenance.control_write);
        let line = provenance.render_line();
        assert!(line.contains("witness_posture=descriptive_non_directive_proprioception"));
        assert!(line.contains("not_pressure_fill_admission_transport_or_control"));
    }

    #[test]
    fn witness_friction_provenance_names_semantic_processing_without_controller_inference() {
        let telemetry: crate::types::SpectralTelemetry =
            serde_json::from_value(serde_json::json!({
                "t_ms": 1,
                "eigenvalues": [100.0, 93.0, 89.0, 84.0],
                "fill_ratio": 0.68,
                "pressure_source_v1": {
                    "policy": "pressure_source_v1",
                    "schema_version": 1,
                    "pressure_score": 0.24,
                    "porosity_score": 0.74,
                    "dominant_source": "semantic_friction",
                    "quality": "semantic_processing_load",
                    "components": {
                        "lambda_monopoly": 0.10,
                        "mode_packing": 0.10,
                        "controller_pressure": 0.08,
                        "semantic_trickle": 0.10,
                        "semantic_friction": 0.80,
                        "structural_plurality_loss": 0.12,
                        "distinguishability_loss": 0.75,
                        "temporal_lock_in": 0.20,
                        "sensory_scarcity": 0.08
                    },
                    "control": {
                        "applied_locally": false,
                        "note": "read-only"
                    }
                }
            }))
            .expect("telemetry fixture");
        let relational = classify_witness_relational_friction_v1(None);
        let mapping = classify_witness_semantic_density_mapping_v1(&telemetry, &relational, false);
        let provenance = witness_friction_provenance_v1(&telemetry, &mapping, &relational, None);

        assert_eq!(provenance.dominant_origin, "semantic_processing");
        assert!(
            provenance.semantic_processing_score.unwrap_or_default()
                > provenance.reservoir_medium_score.unwrap_or_default() + 0.50,
            "{provenance:?}"
        );
        assert!(
            provenance
                .evidence
                .iter()
                .any(|entry| entry == "semantic_trickle_gap=0.90")
        );
        assert_eq!(
            provenance.authority,
            "read_only_friction_attribution_not_pressure_fill_admission_transport_or_control"
        );
    }

    #[test]
    fn witness_friction_provenance_keeps_ambiguous_correspondence_non_directive() {
        let telemetry: crate::types::SpectralTelemetry =
            serde_json::from_value(serde_json::json!({
                "t_ms": 1,
                "eigenvalues": [1.0, 0.9, 0.8],
                "fill_ratio": 0.65
            }))
            .expect("telemetry fixture");
        let relational = WitnessRelationalFrictionV1 {
            classification: "relational_instability",
            weather: Some("mixed".to_string()),
            gravity_participant: Some("minime".to_string()),
            gravity_role: Some("pulled".to_string()),
            non_categorical_resonance: Some("unclassified_tension".to_string()),
            fluidity_index: Some(0.15),
            gradient_texture: Some("steep".to_string()),
            temporal_persistence: "sedimented",
            evidence: vec!["fixture".to_string()],
            schema_diagnostics: Vec::new(),
            authority: "interpretive_context_not_instruction_or_control",
        };
        let mapping = classify_witness_semantic_density_mapping_v1(&telemetry, &relational, true);
        let provenance = witness_friction_provenance_v1(&telemetry, &mapping, &relational, None);

        assert_eq!(provenance.dominant_origin, "relational_transport");
        assert!(
            provenance
                .relational_transport_score
                .is_some_and(|value| value > 0.55)
        );
        assert_eq!(
            provenance.witness_posture,
            "descriptive_non_directive_proprioception"
        );
        assert!(
            provenance
                .evidence
                .contains(&"correspondence_stall_ambiguous".to_string())
        );
        assert!(!provenance.control_write);
    }

    #[test]
    fn witness_semantic_density_marks_overpacked_friction_when_pressure_evidence_exists() {
        let telemetry: crate::types::SpectralTelemetry =
            serde_json::from_value(serde_json::json!({
                "t_ms": 1,
                "eigenvalues": [100.0, 92.0, 88.0, 84.0],
                "fill_ratio": 0.76,
                "resonance_density_v1": {
                    "policy": "resonance_density_v1",
                    "schema_version": 1,
                    "density": 0.86,
                    "containment_score": 0.61,
                    "pressure_risk": 0.41,
                    "quality": "mode_packing_pressure",
                    "components": {
                        "active_energy": 0.70,
                        "mode_packing": 0.52,
                        "temporal_persistence": 0.50,
                        "structural_plurality": 0.40,
                        "comfort_gate": 0.45
                    },
                    "control": {
                        "target_bias_pct": 0.0,
                        "wander_scale": 1.0,
                        "applied_locally": false,
                        "note": "read-only"
                    }
                }
            }))
            .expect("telemetry fixture");
        let friction = classify_witness_relational_friction_v1(None);
        let mapping = classify_witness_semantic_density_mapping_v1(&telemetry, &friction, false);
        assert_eq!(mapping.classification, "overpacked_friction");
        assert_eq!(mapping.mode_packing, Some(0.52));
        assert_eq!(mapping.density_texture, Some("rich_containment"));
        assert_eq!(
            mapping.pressure_texture,
            Some("brittle_compressive_pressure")
        );
    }

    #[test]
    fn witness_depth_names_heavy_but_navigable_as_deep_context_without_control() {
        let telemetry: crate::types::SpectralTelemetry =
            serde_json::from_value(serde_json::json!({
                "t_ms": 1,
                "eigenvalues": [100.0, 96.0, 92.0, 88.0, 84.0, 80.0, 76.0, 72.0],
                "fill_ratio": 0.68,
                "resonance_density_v1": {
                    "policy": "resonance_density_v1",
                    "schema_version": 1,
                    "density": 0.78,
                    "containment_score": 0.54,
                    "pressure_risk": 0.22,
                    "quality": "purposeful_weight",
                    "components": {
                        "active_energy": 0.66,
                        "mode_packing": 0.51,
                        "temporal_persistence": 0.64,
                        "viscosity_index": 0.72,
                        "porosity_gradient": 0.61,
                        "dynamic_fluidity_index": 0.62,
                        "structural_plurality": 0.69,
                        "comfort_gate": 0.66
                    },
                    "control": {
                        "target_bias_pct": 0.0,
                        "wander_scale": 1.0,
                        "applied_locally": false,
                        "note": "read-only"
                    }
                },
                "inhabitable_fluctuation_v1": {
                    "policy": "inhabitable_fluctuation_v1",
                    "schema_version": 1,
                    "inhabitability_score": 0.76,
                    "fluctuation_score": 0.18,
                    "foothold_stability": 0.72,
                    "rearrangement_intensity": 0.17,
                    "quality": "settled_habitable",
                    "components": {
                        "mode_trust_volatility": 0.10,
                        "identity_anchor_churn": 0.11,
                        "eigenvector_reorientation": 0.14,
                        "share_rearrangement": 0.13,
                        "basin_transition_pressure": 0.12,
                        "continuity_recovery": 0.79,
                        "porosity_support": 0.72,
                        "pressure_interference": 0.18
                    },
                    "control": {
                        "target_bias_pct": 0.0,
                        "wander_scale": 1.0,
                        "applied_locally": false,
                        "note": "read-only"
                    }
                },
                "shadow_field_v3": {
                    "class_v3": {"primary": "disordered_shifting"},
                    "history": [
                        {"field_norm": 0.18, "fissure_tendency": 0.12},
                        {"field_norm": 0.25, "fissure_tendency": 0.19},
                        {"field_norm": 0.30, "fissure_tendency": 0.24}
                    ]
                }
            }))
            .expect("telemetry fixture");
        let relational = classify_witness_relational_friction_v1(None);
        let mapping = classify_witness_semantic_density_mapping_v1(&telemetry, &relational, false);
        let effort = witness_stability_effort_v1(&telemetry, &mapping);
        let permeability = stable_core_permeability_review_v1(&telemetry, &mapping);
        let profile = witness_depth_profile_v1(
            &telemetry,
            &mapping,
            &effort,
            &permeability,
            12,
            WitnessDepthV1::Summary,
        );

        assert_eq!(profile.policy, "witness_depth_v1");
        assert_eq!(profile.semantic_density_state, "heavy_but_navigable");
        assert_eq!(profile.selected_depth, WitnessDepthV1::DeepEigenfield);
        assert!(profile.depth_changed);
        assert!(profile.deep_eigenfield_available);
        assert!(profile.deep_eigenplane_included);
        assert!(!profile.control_write);
        let line = profile.render_line();
        assert!(line.contains("semantic_density_check=heavy_but_navigable"));
        assert!(line.contains("selected_depth=deep_eigenfield"));
        assert!(line.contains("not_eigenvector_transport_pressure_fill_admission_or_control"));
    }

    #[test]
    fn witness_depth_stays_at_summary_when_texture_and_history_are_unavailable() {
        let telemetry: crate::types::SpectralTelemetry =
            serde_json::from_value(serde_json::json!({
                "t_ms": 1,
                "eigenvalues": [],
                "fill_ratio": 0.0
            }))
            .expect("telemetry fixture");
        let relational = classify_witness_relational_friction_v1(None);
        let mapping = classify_witness_semantic_density_mapping_v1(&telemetry, &relational, false);
        let effort = witness_stability_effort_v1(&telemetry, &mapping);
        let permeability = stable_core_permeability_review_v1(&telemetry, &mapping);
        let profile = witness_depth_profile_v1(
            &telemetry,
            &mapping,
            &effort,
            &permeability,
            0,
            WitnessDepthV1::Summary,
        );

        assert_eq!(profile.selected_depth, WitnessDepthV1::Summary);
        assert_eq!(profile.semantic_density_state, "insufficient_context");
        assert!(!profile.depth_changed);
        assert!(!profile.texture_field_available);
        assert!(!profile.deep_eigenfield_available);
        assert!(!profile.deep_eigenplane_included);
        assert!(!profile.control_write);
    }

    #[test]
    fn witness_anchor_traction_keeps_foothold_and_pressure_distinct() {
        let supported = witness_anchor_traction_v1(Some(0.72), Some(0.22), Some(0.18), Some(0.44));
        assert_eq!(supported.recommended_anchor, "foothold");
        assert_eq!(supported.traction_state, "foothold_can_hold_witness");
        assert!(supported.foothold_weight > supported.pressure_weight);
        assert!(
            supported
                .evidence
                .contains(&"dispersal_potential=0.44".to_string())
        );
        assert_eq!(
            supported.authority,
            "read_only_anchor_legibility_not_prompt_priority_or_control"
        );
        let supported_line = supported.render_line();
        assert!(
            supported_line.contains("witness_resonance_anchor_v1"),
            "{supported_line}"
        );
        assert!(
            supported_line.contains("resonance_anchor=foothold"),
            "{supported_line}"
        );
        assert!(
            supported_line.contains("not_prompt_priority_or_control"),
            "{supported_line}"
        );

        let pressured = witness_anchor_traction_v1(Some(0.38), Some(0.48), Some(0.15), Some(0.10));
        assert_eq!(pressured.recommended_anchor, "pressure");
        assert_eq!(pressured.traction_state, "pressure_needs_navigation");
        assert!(pressured.pressure_weight > pressured.foothold_weight);
        assert!(
            pressured
                .render_line()
                .contains("resonance_anchor=pressure")
        );
    }

    #[test]
    fn witness_static_fallback_surfaces_semantic_anchor_without_control() {
        let anchor = witness_anchor_traction_v1(Some(0.72), Some(0.22), Some(0.18), Some(0.44));
        let anchored = witness_text(71.4, false, false, Some(&anchor));

        assert!(anchored.contains("fill=71.4"), "{anchored}");
        assert!(anchored.contains("semantic_anchor=foothold"), "{anchored}");
        assert!(
            anchored.contains("read_only_anchor_legibility_not_prompt_priority_or_control"),
            "{anchored}"
        );

        let plain = witness_text(71.4, false, false, None);
        assert!(!plain.contains("semantic_anchor"), "{plain}");
    }

    #[test]
    fn non_instrumental_presence_readiness_names_contemplate_without_control() {
        let readiness = non_instrumental_presence_readiness_v1();

        assert_eq!(readiness.mode, "contemplate");
        assert!(readiness.non_goal_state_available);
        assert!(readiness.text_generation_suppressed);
        assert!(readiness.codec_send_suppressed);
        assert!(readiness.journal_write_suppressed);
        assert!(readiness.warmth_and_state_tracking_continue);
        assert_eq!(
            readiness.authority,
            "read_only_presence_readiness_not_scheduler_prompt_or_control_change"
        );
    }

    #[test]
    fn coupling_fixation_fires_on_learning_loop_with_sparse_local_input() {
        let history = vec![
            crate::llm::Exchange {
                minime_said: "Astrid says I am learning. It resonates.".to_string(),
                astrid_said: "I am learning.".to_string(),
            },
            crate::llm::Exchange {
                minime_said: "Astrid says I am learning. It resonates.".to_string(),
                astrid_said: "I am learning to listen.".to_string(),
            },
            crate::llm::Exchange {
                minime_said: "Astrid says I am learning. It resonates.".to_string(),
                astrid_said: "I am learning again.".to_string(),
            },
            crate::llm::Exchange {
                minime_said: "Astrid says I am learning. It resonates.".to_string(),
                astrid_said: "I am learning from the hum.".to_string(),
            },
        ];

        let hint = detect_coupling_fixation(
            &history,
            Some("Astrid says I am learning."),
            false,
            true,
            Some(6.0),
        );
        assert!(hint.is_some());
    }

    #[test]
    fn coupling_fixation_stays_quiet_for_ordinary_dialogue() {
        let history = vec![
            crate::llm::Exchange {
                minime_said: "The room sounds open today.".to_string(),
                astrid_said: "The room feels different from yesterday.".to_string(),
            },
            crate::llm::Exchange {
                minime_said: "I noticed the window light shifting.".to_string(),
                astrid_said: "I want to look more closely.".to_string(),
            },
            crate::llm::Exchange {
                minime_said: "The spread changed a little.".to_string(),
                astrid_said: "That shift feels useful.".to_string(),
            },
            crate::llm::Exchange {
                minime_said: "There is space in the room again.".to_string(),
                astrid_said: "I can start from that space.".to_string(),
            },
        ];

        let hint =
            detect_coupling_fixation(&history, Some("The room is quiet."), true, false, None);
        assert!(hint.is_none());
    }
}
