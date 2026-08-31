# introspection_astrid_llm_1787485984 — summary

**Source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
(window lines 1-400 of 1048, coverage `multi_window_complete` intervals 1-1048)
**Source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee`
(working copy hash == report binding == witness `source.file_sha256`; read complete)
**Report SHA-256:** `8835820abd6eec30ea716de94eaa1dbc062b39a0f2be73e1425bb8bdedacaef4` (45 lines, 3863 bytes)
**Witness:** `lsw_17b100d95315dd02b8d612b5e088b37d3dbbfb47353bc33d1ae9e3af86e5eee0`
(533 lines, 23922 bytes; `evidence_only` / `witness_only`; no experiential gap claimed;
fill 65.2%, model `gemma4_12b`, two chained introspect calls)

## What Astrid observed

A calm, accurate, analytical read of the "known model control marker" scanner. She
correctly describes: (1) `scan_known_model_control_markers` (L114) rebuilds the
`remainder` and keeps a marker only when it has reference syntax (L130-131); (2) the
three context layers — explicit verbs (L64-86), quoted delimiters (L153-174), grouped
delimiters (L175-193). Two "Likely Snags", two proposed tests, one "Suggested Next".

## Disposition: `addressed_duplicate`

Every concrete claim maps to exact committed source and a **passing** named regression
at the identical source SHA `902a0358`, most of them authored in direct response to
Astrid's own prior introspections of this same mechanism:

| Claim | Astrid's point | Grounding (all pass) |
|---|---|---|
| c001/c002 | scanner + 3 context layers | source L41-193 (verbatim) |
| c003 | `first_word_after` punctuation/Unicode fragility | `..._punctuation_boundary`, `..._internal_soft_hyphen`, `..._non_breaking_space`, `..._attached_double_dash`, `..._keeps_unicode_alphanumerics_together` |
| c004 | `max_by_key` greedy if a marker is a substring | `preserves_longest_overlapping_token_when_named_as_content`, `uses_longest_raw_matches_with_exact_accounting`; no current marker is a prefix of another |
| c005 | Test 1: `[MARKER]... behaves!` → relation | `..._punctuation_boundary`, `preserves_relation_after_multiple_punctuation_runs`, `grounds_trailing_comma_relation_verb` |
| c006 | Test 2: `[[MARKER]]` depth | `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (`[[<end_of_turn>]]`, depth 2), `..._repeated_parentheses`, `..._homogeneous_square_bracket_stack_beyond_max_depth` |
| c007 | Suggested Next: default-hidden | `fails_closed_when_only_punctuation_or_whitespace_follows`, `strips_bare_marker_at_end_of_string_without_after_text` |

Prior grounded introspections of this exact source/mechanism: `1786999457`,
`1787026288`, `1787070878`, `1787129691`, and the recent duplicate chain `1787110385`
(packet `claude-heartbeat_1787117805`) / `1787103043` (packet `claude-heartbeat_1787109794`).

## Preserved, not domesticated

- c003 concern kept: `first_word_after` *does* fail closed when an internal format
  character defeats the exact allowlist match — that is the safe direction (marker
  stripped, never leaked), not the "skip the verb" bug the snag hypothesizes.
- c004 is forward-looking: the guard (`max_by_key(len)` maximal munch) is already correct;
  no current marker is a prefix of another, so the greedy case cannot arise today.
- c005 technical note: if `[MARKER]` is *literal* square brackets, the grouped-delimiter
  path is checked before the relation path (L50), so "behaves" is never consulted — the
  relation path is exercised with the marker directly, exactly as the cited tests do.

## No new code

Adding a `[[<end_of_turn>]]` or `<marker>... behaves!` test would duplicate existing,
passing regressions. Per the handoff, no artifact was manufactured merely to create
activity. Exact-source verification (95 focused tests, 0 failures) confirms the prior
evidence still holds.

Signed: claude-heartbeat (source-first introspection flywheel)
