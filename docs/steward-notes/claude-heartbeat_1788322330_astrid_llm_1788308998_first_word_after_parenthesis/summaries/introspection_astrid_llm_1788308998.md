# Summary — introspection_astrid_llm_1788308998

Astrid read `astrid:llm` (`dialogue_runtime.rs`, full file 1-1048, source SHA
`902a0358…`, matching the report/witness binding) and described the
model-control-marker scanner: used-as-command vs referenced-as-string, with
`scan_known_model_control_markers` (L114), `reference_syntax` (L49), delimiter
punctuation (L153-197), and relation verbs like "behaves"/"represents"
(L64-86). All five line citations verified exact against complete source.

- **Likely Snag** — `first_word_after` (L89) on a parenthetical `[MARKER] (as a
  test)` "might fail to identify 'as'." **Source says it does not fail**: the
  per-chunk `trim_matches(|c| !alnum && c != '_')` strips the leading `(` fused
  inside the chunk `(as`, yielding `as`. This is a *distinct* mechanism from the
  existing `--` (L2724) and `:` (L2770) regressions, where the punctuation is a
  separate whitespace chunk that collapses to empty and is skipped by
  `find(!empty)`. No existing test placed a delimiter *fused* to the relation
  word, so a focused regression was added
  (`control_marker_cleanup_first_word_after_trims_fused_leading_parenthesis`):
  listed `as` keeps the marker byte-exact; unlisted `acts` strips it, isolating
  the gate to allowlist membership, not the `(`. Passes.
- **Test 1 (delimiter depth `[[MARKER]]` + MAX)** — already covered
  (`…double_square_bracket_depth_two` L3280; four-level L2194; beyond-max L2301;
  `MAX_EXACT_REFERENCE_DELIMITER_DEPTH=4` at L151).
- **Test 2 (`behaves` relation preserved in remainder)** — already covered
  (`scan_…preserves_grouped_and_explicit_relation_contexts` L3200-3210).
- **Suggested Next (double-sanitization of "minime" tokens)** — no interaction
  by construction: `sanitize_minime_context_for_dialogue` (L530) is a line-level
  filter on *prompt input*; `scan_known_model_control_markers` is a byte-level
  sanitizer on *Astrid's output*; disjoint data paths, neither calls the other,
  and `minime` is not a known control marker.

No felt-experience gap was claimed (witness
`lived_state_experiential_gap_claimed=false`). No live/substrate change implied
or authorized. The one durable change is a non-live focused test.

**Round outcome:** analysis and the test are complete and verified, but the
addressing close sequence and the Division record-round were **not** run — only
~7 min of the 5400s child cap remained after analysis (preprojection + cold
`cargo` compiles consumed the budget). The report therefore remains **unread /
not closed** in the addressing system and is left at the queue head for the next
cycle, which can close it citing the four `verified_existing` dispositions plus
the newly added regression.
