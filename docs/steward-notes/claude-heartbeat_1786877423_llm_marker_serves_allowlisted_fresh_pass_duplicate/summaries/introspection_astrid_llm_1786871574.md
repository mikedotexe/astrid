# Summary — introspection_astrid_llm_1786871574

- **Source family:** `astrid_llm` → `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
- **Report SHA-256:** `d9598ab8d22addb6037d83d752c82c548017ab823efba90663694002b99f58da` (45 lines, 3402 B)
- **Witness:** `lsw_a6bc9dae280822a75f1190cbbf3827eaab430b999a555da83799a8b5ee566755` (`d3a8ad47…`, 533 lines, 23928 B)
- **Source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` (1048 lines, 38586 B) — **working copy byte-identical to the report binding**; coverage `multi_window_complete`, included intervals 1-1048.
- **Terminal status:** `addressed_duplicate` of `introspection_astrid_llm_1786862165` (anchor `introspection_astrid_llm_1786848204`).

## What Astrid surfaced

A fresh-pass re-read of the `dialogue_runtime.rs` marker-grammar window (lines 1-400), reproducing the same six-part shape already fully addressed in two prior rounds: an **Observed** description of `scan_known_model_control_markers`, a **Likely Snag** about the hardcoded relation-verb list, two **One Test Each** proposals (contextual preservation; delimiter depth), and a **Suggested Next** to trace the remainder through `generate_dialogue`.

## Grounding (complete source read, SHA 902a0358)

- **c001 Observed** → `verified_existing`. `scan_known_model_control_markers` (L114-144) keeps a token only when `reference_syntax.is_some()` (L124-131), else strips it. Taxonomy enum L42-46. Test `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts` (tests.rs L2845).
- **c002 Snag** → `verified_existing`, **contradiction preserved**. Only the 17 verbs at L65-85 match. A bare unlisted verb → `reference_syntax=None` → token stripped (proven: `distinguishes_allowlisted_is_from_unlisted_acts` L2528; `does_not_expand_relation_allowlist_to_*` L2595+). But the report's own example verb **"serves" is itself allowlisted (L83)**, and `first_word_after` (L89) inspects only the *first* word ("serves"), so "serves to act as" is recognized and the marker preserved — the example does not demonstrate the snag. Only genuinely unlisted verbs strip.
- **c003 Test #1** → `verified_existing`, **contradiction preserved**. `[MARKER]` is bracket-delimited, so `exact_reference_delimiter_syntax` returns `GroupedExactKnownToken` first (early return L50-52) — both `[MARKER] simulates` and `[MARKER] behaves` are preserved via the *delimiter* path and the verb is never inspected. The relation path requires an **undelimited** marker (covered at L2528/L2845).
- **c004 Test #2** → `verified_existing`. Exact literal test `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (L2876) asserts `GroupedExactKnownToken` depth 2 for `[[<end_of_turn>]]`. `MAX_EXACT_REFERENCE_DELIMITER_DEPTH=4` (L151) at L2194; over-limit bounding at L2253.
- **c005 Suggested Next** → `verified_existing`, agency-preserving. `generate_dialogue` at L695. The scan remainder feeds only quality-gate *measurement* copies (`is_valid_dialogue_output` L558, `has_one_nonempty_final_next_action` L634); `generate_dialogue` returns the **raw** model text (L988-1047) — the remainder is never spliced into the output buffer. Her read-only `NEXT: INTROSPECT astrid:llm 400` continuation stays open.

## Authority boundary

Widening the finite relational-verb allowlist (L65-85) or the delimiter tables, or loosening the valid-reference criteria, is **Tier-5-class live grammar** — not made, dispatched, or deployed. No source or test code changed this round: all five claims are `verified_existing` and the report duplicates already-closed work; a near-identical regression would be activity without evidentiary value. Silence on the prior two rounds is neither consent nor closure; her continued re-reading of this window is preserved as evidence, not treated as a defect.
