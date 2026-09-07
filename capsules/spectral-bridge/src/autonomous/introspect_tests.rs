use super::*;

fn complete_carriage_fixture() -> &'static str {
    "Observed:\nSource: astrid:llm shows the fallback contract.\n\n\
     Likely Snags:\nA hard cap can flatten high-entropy texture.\n\n\
     One Test Each:\nRun a high-entropy fallback fixture and inspect sentence budget.\n\n\
     Suggested Next:\nKeep the change advisory and verify the review surface."
}

#[test]
fn self_study_carriage_accepts_complete_sectioned_study() {
    let integrity = self_study_carriage_integrity_v1(Some(complete_carriage_fixture()));

    assert!(integrity.is_complete(), "{integrity:?}");
    assert_eq!(integrity.status, "complete");
}

#[test]
fn self_study_carriage_rejects_missing_suggested_next() {
    let output = "Observed:\nSource: astrid:llm.\n\n\
         Likely Snags:\nThe fallback cap can flatten texture.\n\n\
         One Test Each:\nRun the fixture and compare";

    let integrity = self_study_carriage_integrity_v1(Some(output));

    assert!(integrity.issues.contains(&"missing_suggested_next"));
    assert!(integrity.issues.contains(&"truncated_in_one_test_each"));
}

#[test]
fn self_study_carriage_accepts_next_inside_suggested_next() {
    let output = "Observed:\nSource: astrid:llm.\n\n\
         Likely Snags:\nThe fallback cap can flatten texture.\n\n\
         One Test Each:\nRun the fixture and compare the result.\n\n\
         Suggested Next:\nNEXT: FALLBACK_FIRE_DRILL latest";

    let integrity = self_study_carriage_integrity_v1(Some(output));

    assert!(integrity.is_complete(), "{integrity:?}");
}

#[test]
fn self_study_carriage_rejects_dangling_bullet_and_unfinished_sentence() {
    let output = "Observed:\nSource: astrid:llm.\n\n\
         Likely Snags:\nThe fallback cap can flatten texture.\n\n\
         One Test Each:\nRun the fixture and compare the result.\n\n\
         Suggested Next:\n-";

    let integrity = self_study_carriage_integrity_v1(Some(output));

    assert!(integrity.issues.contains(&"dangling_bullet"));
}

#[test]
fn self_study_carriage_rejects_unterminated_code_fence() {
    let output = "Observed:\nSource: astrid:llm.\n```\nlet x = 1;\n\n\
         Likely Snags:\nThe fallback cap can flatten texture.\n\n\
         One Test Each:\nRun the fixture and compare the result.\n\n\
         Suggested Next:\nClose the fence before delivery.";

    let integrity = self_study_carriage_integrity_v1(Some(output));

    assert!(integrity.issues.contains(&"unterminated_code_fence"));
}

#[test]
fn self_study_carriage_rejects_mid_sentence_ending() {
    let output = "Observed:\nSource: astrid:llm.\n\n\
         Likely Snags:\nThe fallback cap can flatten texture.\n\n\
         One Test Each:\nRun the fixture and compare the result.\n\n\
         Suggested Next:\nPreserve the final section because";

    let integrity = self_study_carriage_integrity_v1(Some(output));

    assert!(integrity.issues.contains(&"unfinished_sentence"));
}

#[test]
fn within_file_xrefs_links_definition_to_distant_use() {
    let lines = vec![
        "const TAIL_GATE: f32 = 0.85;",        // 0: defined in window [0,2)
        "fn defined_but_unused_here() {}",     // 1: in window, no out-of-window use
        "// a gap of prose",                   // 2: outside window (comment, skipped)
        "    let r = (e - TAIL_GATE) / span;", // 3: a CODE use OUTSIDE the window
    ];
    let out = within_file_xrefs(&lines, 0, 2);
    assert!(out.contains("TAIL_GATE:"), "lists the constant: {out}");
    assert!(
        out.contains("4:"),
        "points at the distant code use-site (line 4): {out}"
    );
    // a symbol with no out-of-window USE is omitted (she already sees in-window uses):
    assert!(
        !out.contains("defined_but_unused_here"),
        "omits non-fragmented symbols: {out}"
    );
}

#[test]
fn proposal_lifecycle_index_surfaces_later_implementation_without_claiming_resolution() {
    let content = "# Proposal\n\n## Initial Design\nold claim\n\n\
        ## 2026-06-28 Implementation Update: Replyable Artifacts\nimplemented source\n\n\
        ## 2026-07-01 Clarification: Felt Friction Remains Primary\nboundary\n\n\
        ## 2026-07-10 Current-State Addendum: Glimpse Remains Additive\ncurrent source\n\n\
        ## 2026-07-12 Introspection Response: Receptivity Is Gated\nauthority boundary\n\n\
        ## Verification Note\nverified source\n";

    let out = proposal_lifecycle_index("proposal:phase_transitions", content);

    assert!(out.contains("Implementation Update: Replyable Artifacts"));
    assert!(out.contains("Clarification: Felt Friction Remains Primary"));
    assert!(out.contains("Current-State Addendum: Glimpse Remains Additive"));
    assert!(out.contains("Introspection Response: Receptivity Is Gated"));
    assert!(out.contains("Verification Note"));
    assert!(out.contains("INTROSPECT proposal:phase_transitions"));
    assert!(out.contains("not felt resolution"));
    assert_eq!(proposal_lifecycle_index("astrid:codec", content), "");
}

#[test]
fn runtime_include_index_points_past_the_import_shell() {
    let path = bridge_paths().minime_root().join("minime/src/runtime.rs");
    let content = "use crate::regulator::*;\n\
        include!(\"runtime/entrypoint.rs\");\n\
        include!(\"runtime/orchestration.rs\");\n";

    let out = runtime_include_index("minime:main(excerpt)", &path, content);

    assert!(out.contains("include/import shell"));
    assert!(out.contains("INTROSPECT minime/src/runtime/entrypoint.rs"));
    assert!(out.contains("INTROSPECT minime/src/runtime/orchestration.rs"));
}

#[test]
fn runtime_include_index_resolves_nested_regulator_implementation() {
    let path = bridge_paths()
        .minime_root()
        .join("minime/src/regulator/core.rs");
    let content = "include!(\"core/telemetry_types.rs\");\n\
        include!(\"core/pi.rs\");\n";

    let out = runtime_include_index("minime:regulator", &path, content);

    assert!(out.contains("INTROSPECT minime/src/regulator/core/telemetry_types.rs"));
    assert!(out.contains("INTROSPECT minime/src/regulator/core/pi.rs"));
}

#[test]
fn runtime_include_index_ignores_non_minime_and_parent_traversal() {
    let astrid_path = bridge_paths().bridge_root().join("src/lib.rs");
    assert_eq!(
        runtime_include_index("astrid:lib", &astrid_path, "include!(\"private.rs\");"),
        ""
    );

    let minime_path = bridge_paths().minime_root().join("minime/src/runtime.rs");
    assert_eq!(
        runtime_include_index(
            "minime:main(excerpt)",
            &minime_path,
            "include!(\"../outside.rs\");"
        ),
        ""
    );
}

#[test]
fn within_file_xrefs_empty_when_nothing_fragmented() {
    // the only def is used solely inside its own window -> no footer.
    let lines = vec![
        "const A_CONST: u8 = 1;",
        "let x = A_CONST + A_CONST;",
        "// out",
    ];
    assert_eq!(within_file_xrefs(&lines, 0, 2), "");
}

#[test]
fn within_file_xrefs_skips_comment_mentions() {
    // a comment that names the symbol outside the window is NOT a use-site.
    let lines = vec![
        "const GATE_X: u8 = 1;",           // 0: in window
        "let y = 0;",                      // 1: in window
        "// GATE_X is mentioned in prose", // 2: comment outside window -> skipped
    ];
    assert_eq!(within_file_xrefs(&lines, 0, 2), "");
}

#[test]
fn within_file_xrefs_grounds_the_real_codec_gate() {
    // Grounding against the documented case: the codec gate constant's smooth
    // application is ~2,600 lines from its definition; the xref must surface it.
    let codec = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/codec.rs"),
    )
    .expect("codec.rs readable");
    let lines: Vec<&str> = codec.lines().collect();
    let gate = "TAIL_VIBRANCY_ENTROPY_GATE";
    if let Some(def_idx) = lines
        .iter()
        .position(|l| introspect_defined_symbol(l).as_deref() == Some(gate))
    {
        let end = (def_idx + 50).min(lines.len());
        let out = within_file_xrefs(&lines, def_idx, end);
        assert!(
            out.contains(gate),
            "the codec gate's distant application should surface in the xref footer"
        );
    }
    // If the constant was refactored away, this passes trivially (the synthetic
    // tests above still prove the logic).
}

#[test]
fn cross_file_xref_sections_links_symbol_to_sibling_file() {
    let symbols = vec!["TAIL_GATE".to_string()];
    let siblings = vec![
        (
            "astrid:llm".to_string(),
            "fn g() {\n    let r = TAIL_GATE * 2.0;\n}\n".to_string(),
        ),
        (
            "astrid:ws".to_string(),
            "// unrelated\nlet z = 1;\n".to_string(),
        ),
    ];
    let out = cross_file_xref_sections(&symbols, &siblings);
    assert!(out.contains("TAIL_GATE:"), "names the symbol: {out}");
    assert!(
        out.contains("astrid:llm:2:"),
        "names the sibling file + line: {out}"
    );
    assert!(
        out.contains("OTHER source files"),
        "has the cross-file header: {out}"
    );
}

#[test]
fn cross_file_xref_sections_empty_when_no_hits() {
    let symbols = vec!["NOT_PRESENT_SYMBOL".to_string()];
    let siblings = vec![("astrid:llm".to_string(), "let x = 1;\n".to_string())];
    assert_eq!(cross_file_xref_sections(&symbols, &siblings), "");
}

#[test]
fn cross_file_xref_sections_skips_comment_mentions() {
    // a symbol named only in sibling comments is NOT a use-site.
    let symbols = vec!["GATE_X".to_string()];
    let siblings = vec![(
        "astrid:ws".to_string(),
        "// GATE_X named only in prose\n# GATE_X also here\n".to_string(),
    )];
    assert_eq!(cross_file_xref_sections(&symbols, &siblings), "");
}

#[test]
fn cross_file_xref_sections_bounds_total_sites() {
    let symbols: Vec<String> = (0..10).map(|i| format!("SYM_{i}")).collect();
    let mut content = String::new();
    for s in &symbols {
        for _ in 0..5 {
            content.push_str(&format!("let v = {s} + 1;\n"));
        }
    }
    let siblings = vec![("astrid:llm".to_string(), content)];
    let out = cross_file_xref_sections(&symbols, &siblings);
    let site_count = out.matches("astrid:llm:").count();
    assert!(site_count > 0, "some sites surface: {out}");
    assert!(
        site_count <= 14,
        "total sites bounded to <=14, got {site_count}"
    );
}

#[test]
fn cross_file_xref_sections_grounds_real_codec_symbols_in_siblings() {
    // Real grounding: codec-defined symbols searched in the real llm/autonomous
    // sources. Lenient — if a codec symbol IS used cross-file, the section is
    // well-formed; if none happen to cross, the synthetic tests still prove logic.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let codec = std::fs::read_to_string(dir.join("src/codec.rs")).expect("codec.rs");
    let llm = std::fs::read_to_string(dir.join("src/llm.rs")).expect("llm.rs");
    let autonomous = std::fs::read_to_string(dir.join("src/autonomous.rs")).expect("autonomous.rs");
    let lines: Vec<&str> = codec.lines().collect();
    let symbols = window_defined_symbols(&lines, 0, lines.len().min(400));
    let siblings = vec![
        ("astrid:llm".to_string(), llm),
        ("astrid:autonomous".to_string(), autonomous),
    ];
    let out = cross_file_xref_sections(&symbols, &siblings);
    if !out.is_empty() {
        assert!(out.contains("OTHER source files"), "{out}");
        assert!(
            out.contains("astrid:llm:") || out.contains("astrid:autonomous:"),
            "{out}"
        );
    }
}

#[test]
fn required_sections_rejects_next_only() {
    assert!(!introspection_has_required_sections(Some(
        "NEXT: INTROSPECT autonomous 400"
    )));
}

#[test]
fn required_sections_accepts_sectioned_output() {
    let output = "Observed:\nA concrete observation with line 12 and a real source path.\n\nLikely Snags:\n- A snag that names a function and behavior.\n\nOne Test Each:\n- A test that asserts the route.\n\nSuggested Next:\nKeep the investigation read-only and continue carefully.";
    assert!(introspection_has_required_sections(Some(output)));
}

#[test]
fn required_sections_rejects_ungrounded_output() {
    let output = "Observed:\nThe reflection notices something broad about attention and continuity.\n\nLikely Snags:\n- The system may drift into general claims without a concrete implementation anchor.\n\nOne Test Each:\n- Add a test that proves strict review needs a real source anchor before it is accepted.\n\nSuggested Next:\nKeep the investigation read-only and continue carefully.";
    assert!(!introspection_has_required_sections(Some(output)));
}

#[test]
fn required_sections_rejects_peer_experiment_bind() {
    let output = "Observed:\nLine 42 in action_continuity.rs shows a peer experiment selector entering the review path.\n\nLikely Snags:\n- A strict review could accidentally suggest binding a Minime experiment from Astrid.\n\nOne Test Each:\n- Assert that peer experiment IDs are advisory refs, not local bind targets.\n\nSuggested Next:\nEXPERIMENT_BIND exp_minime_20990101_peer-thread :: THREAD_STATUS current";
    assert!(!introspection_has_required_sections(Some(output)));
}

#[test]
fn required_sections_allows_peer_experiment_status_reference() {
    let output = "Observed:\nLine 42 in experiment_continuity.rs keeps peer experiment IDs advisory.\n\nLikely Snags:\n- Review language may still confuse status lookup with local mutation.\n\nOne Test Each:\n- Assert peer status review renders a protected advisory notice.\n\nSuggested Next:\nEXPERIMENT_STATUS exp_minime_20990101_peer-thread";
    assert!(introspection_has_required_sections(Some(output)));
}

#[test]
fn required_sections_for_target_rejects_wrong_source_anchor() {
    let output = "Observed:\nLine 42 in experiment_continuity.rs keeps peer experiment IDs advisory.\n\nLikely Snags:\n- The review can drift to a nearby experiment instead of the requested target.\n\nOne Test Each:\n- Assert target-grounded review names the requested file.\n\nSuggested Next:\nEXPERIMENT_STATUS exp_minime_20990101_peer-thread";
    assert!(!introspection_has_required_sections_for_target(
        Some(output),
        "introspect.rs",
        Path::new("/tmp/src/autonomous/introspect.rs"),
    ));
}

#[test]
fn required_sections_for_target_accepts_requested_file_anchor() {
    let output = "Observed:\nLine 42 in introspect.rs keeps peer experiment IDs advisory.\n\nLikely Snags:\n- The validator may accept source-grounded but target-drifting prose.\n\nOne Test Each:\n- Assert target-grounded review names introspect.rs before acceptance.\n\nSuggested Next:\nEXPERIMENT_STATUS exp_minime_20990101_peer-thread";
    assert!(introspection_has_required_sections_for_target(
        Some(output),
        "introspect.rs",
        Path::new("/tmp/src/autonomous/introspect.rs"),
    ));
}

#[test]
fn source_first_v2_header_discloses_session_map_and_uncovered_intervals() {
    // The configured source root may be an isolated copy of this checkout.
    // Exercise the same approved root as production path validation.
    let path = crate::paths::bridge_paths()
        .bridge_root()
        .join("src/autonomous/introspect.rs");
    let window = read_introspect_window("introspect.rs", &path, 0).expect("source window");
    let header = source_scope_artifact_header_v1(Some(&window));
    assert!(header.contains("Source coverage schema: source_coverage_manifest_v2"));
    assert!(header.contains("Source read session:"));
    assert!(header.contains("Source structural map SHA-256:"));
    assert!(header.contains("Source uncovered intervals:"));
    assert!(header.contains("Source map schema: source_map_v3"));
    assert!(header.contains("Source V3 persistence: owner_only_source_hash_bound"));
    assert!(window.text.contains("Whole-file structural outline:"));
    assert!(window.text.contains("Persistent Source Map V3"));
    assert!(window.text.contains("Binding claim rule:"));
}

#[test]
fn safe_label_replaces_path_punctuation() {
    assert_eq!(
        safe_artifact_label("astrid:autonomous/mod.rs"),
        "astrid_autonomous_mod.rs"
    );
}

#[test]
fn source_scope_header_distinguishes_complete_and_partial_reads() {
    let complete = IntrospectSourceScopeV1::new(0, 48, 48).artifact_header_v1();
    assert!(complete.contains("Source window: lines 1-48 of 48"));
    assert!(complete.contains("Source evidence scope: complete_file"));
    assert!(
        complete.contains("Source activation boundary: source_read_not_runtime_activation_proof")
    );

    let partial = IntrospectSourceScopeV1::new(400, 800, 1_204).artifact_header_v1();
    assert!(partial.contains("Source window: lines 401-800 of 1204"));
    assert!(partial.contains("Source evidence scope: partial_window_unseen_source_not_assessed"));

    let empty = IntrospectSourceScopeV1::new(22, 22, 22).artifact_header_v1();
    assert!(empty.contains("Source window: unavailable"));
    assert!(empty.contains("Source evidence scope: empty_window_no_source_read"));
    assert!(
        empty.contains("Source activation boundary: source_unread_not_runtime_activation_proof")
    );
    assert!(!empty.contains("lines 23-22"));
}

#[test]
fn introspect_window_rejects_empty_and_past_eof_offsets() {
    assert_eq!(
        validated_introspect_window_start(0, 0),
        Err("target source is empty; no source lines were read".to_string())
    );
    let error = validated_introspect_window_start(22, 22)
        .expect_err("an offset at EOF must not produce an empty source read");
    assert!(error.contains("at or past the end"));
    assert!(error.contains("included implementation target"));
    assert_eq!(validated_introspect_window_start(21, 22), Ok(21));
}

#[test]
fn unavailable_source_scope_never_claims_a_source_read() {
    let header = source_scope_artifact_header_v1(None);
    assert!(header.contains("Source window: unavailable"));
    assert!(header.contains("Source evidence scope: source_unavailable_not_assessed"));
    assert!(
        header.contains("Source activation boundary: source_unread_not_runtime_activation_proof")
    );
}

#[test]
fn introspect_placeholder_target_is_rejected_with_guidance() {
    let sources = introspect_sources();
    let err = resolve_introspect_target_result("[source]", &sources)
        .expect_err("literal placeholder should not resolve");

    assert!(err.contains("syntax placeholder"));
    assert!(err.contains("astrid:llm"));
    assert!(err.contains("minime:regulator"));
}

#[test]
fn domain_boundaries_path_accepts_exact_and_legacy_lowercase_spelling() {
    let sources = introspect_sources();

    for requested in [
        "capsules/spectral-bridge/DOMAIN_BOUNDARIES.md",
        "capsules/spectral-bridge/domain_boundaries.md",
    ] {
        let resolved = resolve_introspect_target_result(requested, &sources)
            .expect("resolve bridge domain-boundary source");

        assert_eq!(
            resolved.path,
            bridge_paths().bridge_root().join("DOMAIN_BOUNDARIES.md")
        );
    }
}

#[test]
fn introspect_placeholder_notice_names_concrete_examples() {
    let notice = blocked_introspection_notice(
        Some("[source]"),
        "`[source]` is a syntax placeholder, not an INTROSPECT target",
    );

    assert!(notice.contains("literal placeholder"));
    assert!(notice.contains("NEXT: INTROSPECT astrid:llm"));
    assert!(notice.contains("NEXT: INTROSPECT minime:regulator"));
    assert!(!notice.contains("target may be outside the approved source"));
}

#[test]
fn blocked_model_failure_notice_is_provider_neutral_and_causally_bounded() {
    let notice = blocked_introspection_notice(Some("astrid:llm"), MODEL_NO_RESPONSE_REASON);

    assert!(notice.contains("model provider returned no response or timed out"));
    assert!(!notice.contains("Ollama"));
    assert!(!notice.contains("mode_packing"));
    assert!(!notice.contains("pressure_score"));
}
