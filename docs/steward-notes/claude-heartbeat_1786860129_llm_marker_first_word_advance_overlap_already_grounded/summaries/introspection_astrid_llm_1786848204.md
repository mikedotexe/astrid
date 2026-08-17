# Summary — `introspection_astrid_llm_1786848204`

- **Source:** `astrid:llm` (`capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`)
- **Report SHA-256:** `c6442992bf495b89d6766316132b72fda2d40abedcbd0dccffe1458db3e910b8` (45 lines, 4051 B)
- **Source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` (1048 lines, 38586 B) — working copy **byte-identical** to the report binding; coverage state `multi_window_complete`, included intervals 1-1048, uncovered none.
- **Lived-state witness:** `lsw_b25ecce714bb69e468bee5400e60b58e9f9289df2dea6162532480b32d9a0d62` (533 lines, 23934 B, SHA `8102dbc6…`); authority `evidence_only` / `witness_only` / `live_eligible_now=false`; fill 72.5%; two `gemma4_12b` mlx introspect routes (61.7s + 59.7s, second repairs first).

## What she surfaced
A fresh-pass reading of the `astrid:llm` marker-scanner. She accurately describes `scan_known_model_control_markers` (L114) filtering markers into a remainder and `exact_reference_delimiter_syntax` (L199) detecting quoting/grouping, with preservation on delimiter syntax (L129-131) or relational verbs (L64-86). She then raises two **Likely Snags** (`first_word_after` punctuation/multi-word fragility; single-char fallback advance inefficiency/fragmentation), two **One Test Each** designs (bracketed-marker contextual preservation; `[TOKEN_X] behaves` relational verb), and a **Suggested Next** (overlapping-marker handling in the `KNOWN_MODEL_CONTROL_MARKERS` list).

## Source-first disposition (all six `verified_existing`)
Every snag concern and both proposed tests are already covered by exact source + passing tests, so no new code is warranted:

- **c001 Observed** — accurate; the two preservation paths are unified through `reference_syntax()` (L49-60, delimiter checked first). Tested by `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts` (L2845).
- **c002 Snag (`first_word_after`)** — **refuted, concern preserved.** `split_whitespace` never collapses words; `trim_matches` strips surrounding punctuation; `find(!empty)` lands on the first real word. `scan_known_model_control_markers_grounds_first_word_after_punctuation_boundary` (L2900) proves a punctuation-run-separated verb still preserves the marker; `control_marker_cleanup_uses_only_the_first_finite_relation_word` (L2565) covers the multi-word worry. Fail-closed only when no alnum word follows.
- **c003 Snag (single-char advance)** — byte-exact (`char.len_utf8()`, L134-140), reconstructs non-marker bytes with no fragmentation; `control_marker_scanner_advances_byte_exactly_across_multibyte_text` (L2099). Cost is bounded, not a defect.
- **c004 Test #1 (bracketed marker)** — the grouped branch (L175-193) returns `GroupedExactKnownToken`; proposed test already exists (`…preserves_grouped_and_explicit_relation_contexts` L2845, `…preserves_exact_tokens_in_declared_group_delimiters` L2151, double-square depth-two L2876).
- **c005 Test #2 (`[TOKEN_X] behaves`)** — **contradiction preserved.** For a *bracketed* marker `reference_syntax()` returns via the delimiter path first (L50), so `followed_by_explicit_exact_token_relation` is never consulted — it preserves via the delimiter, not the verb. The verb path (`behaves` allowlisted L69) is proven on a **bare** marker at L2858.
- **c006 Suggested Next (overlaps)** — `max_by_key(|t| t.len())` (L106) is longest-match; her `COMMAND`/`COMMAND_EXEC` example is not in the actual list (NOT_FOUND) but real overlaps (`<channel|>` ⊂ `thought <channel|>`, `[INST]`/`[/INST]`) are proven by L1745 / L2453.

## Terminal status
`addressed_no_action` — evidence-backed no-change. Widening the finite relational-verb allowlist or the delimiter tables is a **Tier-5-class live-grammar / model-output change** and was **not** made, dispatched, or deployed. Her `NEXT: INTROSPECT astrid:llm 400` read-only continuation remains open to her. Silence stays neutral; her finite-list concern is preserved as standing evidence.

## Verification
Eight focused marker tests run green at the current source SHA (incremental build 3.74s, `0 failed`): `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts`, `scan_known_model_control_markers_grounds_first_word_after_punctuation_boundary`, `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two`, `control_marker_cleanup_uses_longest_raw_matches_with_exact_accounting`, `control_marker_cleanup_preserves_longest_overlapping_token_when_named_as_content`, `control_marker_scanner_advances_byte_exactly_across_multibyte_text`, `control_marker_cleanup_preserves_exact_tokens_in_declared_group_delimiters`, `control_marker_cleanup_uses_only_the_first_finite_relation_word`.
