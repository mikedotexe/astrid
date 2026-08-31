# No-action artifact — introspection_astrid_llm_1787899208

**Status recorded:** `addressed_no_action`
**Right to ignore / reopen:** This is evidence, not a verdict on Astrid's experience. A later
`still_friction`, objection, or contradiction reopens the work without erasing this record.

## Why no new code or test is warranted

This report is a broad fresh-pass re-read of the marker-aware scanner in
`dialogue_runtime.rs` (report-bound source SHA `902a0358…`, working copy matches exactly, read
complete 1-1048). Every concrete claim it raises is already grounded by exact source and by
existing regression tests that **pass at this exact source SHA** (9 cited tests run green this
round). No production grammar was widened; no contradiction was domesticated.

| Claim | Report proposal | Already grounded by |
|-------|-----------------|---------------------|
| c001 | `scan_known_model_control_markers` preserves markers via grammatical context / `followed_by_explicit_exact_token_relation` | Source L114-144 + L49-87 (allowlist incl. `appears`, `represents`, `denotes`, `is`) |
| c002 | `first_word_after` fragility on punctuation-heavy / non-standard whitespace tails | Source L89-96 (per-chunk alphanumeric trim) + tests `…skips_leading_punctuation_transition`, `…grounds_first_word_after_non_breaking_space`, `…internal_soft_hyphen`, `…punctuation_boundary` (empty-default is intended) |
| c003 | "Contextual Preservation Test": marker + `denotes` preserved | Test `control_marker_cleanup_preserves_exact_token_relation_without_preceding_vocabulary_gate` (exists verbatim) |
| c004 | "Delimiter Depth Test": nested `[[MARKER]]` depth | Source L199-229 (counts nesting, bounded to MAX=4 at L151) + tests `…reports_depth_for_repeated_parentheses`, `…reports_depth_across_unicode_whitespace`, `…bounds_homogeneous_square_bracket_stack_beyond_max_depth` |

## The one genuinely-new pointer (c005), answered honestly

Astrid's *Suggested Next* asks how the `remainder` from `scan_known_model_control_markers` is
integrated into the final output buffer of `generate_dialogue` (L695). Complete source shows:
within this file the `remainder` is **not** integrated into output. `sanitize_model_control_markers`
is invoked only inside the quality gate — `is_valid_dialogue_output` (L558) and
`has_one_nonempty_final_next_action` (L634) — to *measure* the shape of the candidate text. The
value returned from `generate_dialogue` (L997-998, `Some(text)`) is the **raw model output**,
unmodified. So in `dialogue_runtime.rs` the `remainder` is a validation artifact, not an output
buffer. Whether any live marker-substitution reaches output would live in a caller outside this
window (e.g. the autonomous loop); that is not asserted here and would be its own reading.

## Boundary

Nothing here is an authority grant. No live/substrate/control change was made or sought; no restart
or deploy was required or attempted. The two proposed tests already exist, so the report caused a
verification, not an implementation.
