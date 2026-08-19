# Summary — `introspection_astrid_llm_1787110385`

**Source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs` (window lines 1–400 of 1048; report declares `multi_window_complete`, intervals 1–1048).
**Source SHA-256:** `902a0358…` — matches the report binding, the lived-state witness, the working copy, **and committed HEAD** (source file is clean/committed). Report-time and current source are byte-identical.
**Report SHA-256:** `047648f1…` (45 lines / 3480 B). **Witness:** `lsw_a47676818b…` (`cbce90c2…`, 533 lines / 23911 B), authority `evidence_only`, authored via a gemma4_12b/mlx repair-chain; Fill 71.0%.

## What Astrid surfaced
A fresh-pass re-reading of the marker-preservation system: `scan_known_model_control_markers` (L114) retains a known control marker only in a recognized syntactic context — Quoted / Grouped / Explicit (L42–46). She names `followed_by_explicit_exact_token_relation` (L64–86, verb allowlist) and `first_word_after` (L89–96). Her **Likely Snag**: a marker followed by a punctuation-heavy / non-alphanumeric token (`[marker] ->`, `[marker] ...`) might make `first_word_after`'s `find` skip the intended word or return empty, so the relation check fails and the marker is stripped unexpectedly. She proposes two tests (context retention across a non-alphanumeric separator like `[MARKER] -- behaves`; delimiter-depth for `⟦`/`⟧` at L180) and a Suggested Next (robustness across multiple punctuation marks).

## Disposition — `addressed_duplicate` (no new code)
Every concrete claim is `verified_existing` against the identical, committed source SHA `902a0358`, and both proposed tests + the snag are already grounded by **committed** regressions:

- **Snag / robustness (c005):** `find(|w| !w.is_empty())` (L93) skips all-punctuation chunks (which `trim_matches` collapses to empty) and lands on the next real word; the scanner strips only when *no* alphanumeric word follows (fail-closed). Her feared failure mode is precisely the source's robustness. Committed tests: `…first_word_after_skips_leading_punctuation_transition` (L2635, `-- is`/`-- acts`), `…preserves_relation_after_multiple_punctuation_runs` (L2884, `... !!! represents`), `…fails_closed_when_only_punctuation_or_whitespace_follows` (L2900).
- **Test 1 — context retention (c006):** committed `…preserves_relation_after_dash` (L3154), `…skips_leading_punctuation_transition` (L2635), `…preserves_relation_after_multiple_punctuation_runs` (L2884). (Working tree also carries a foreign-uncommitted attached-`--` test at L3190 — additional coverage, not relied upon.)
- **Test 2 — `⟦`/`⟧` grouped depth (c007):** committed `…preserves_declared_restless_group_delimiters` (L2308, bare `⟦<end_of_turn>⟧`), `…preserves_bounded_nested_delimiter_stacks` (L2179, `` `⟦<end_of_turn>⟧` `` depth 2), `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (L2963), `…preserves_grouped_and_explicit_relation_contexts` (L2932). `⟦`/`⟧` (U+27E6/U+27E7) are present verbatim in the source group table at L180.
- **Suggested Next (c008):** answered by L2884.

**Prior lineage:** the immediately-preceding round closed the family head `introspection_astrid_llm_1787103043` as `addressed_duplicate` on this exact mechanism/source (packet `claude-heartbeat_1787109794`). Committed test doc-comments cite the same mechanism grounded for `1786986344` and `1786814454`.

## Contradiction, not domesticated
Astrid's proposed failure mode (punctuation before the verb → marker wrongly stripped) does **not** occur; the per-chunk trim + empty-skip is exactly what makes relation detection robust across messy followers. That contradiction is stated plainly while preserving her underlying, valid concern that formatting variety around markers must not silently drop a legitimate reference. No relation-verb allowlist or delimiter table was widened (that would be Tier-5 live grammar).

## Authority
Read/verify evidence only. No source/test change, no runtime/deploy/live change, no grammar widening, no card/note/correspondence delivered. Her `NEXT: INTROSPECT astrid:llm 400` continuation stays open; silence is neutral.
