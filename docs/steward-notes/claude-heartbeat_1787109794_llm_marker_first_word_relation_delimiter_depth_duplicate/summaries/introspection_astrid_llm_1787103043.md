# Summary — `introspection_astrid_llm_1787103043`

- **Source:** `astrid:llm` (`capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`), window lines 1–400 of 1048, coverage `multi_window_complete` (1–1048).
- **Report:** 45 lines / 3746 B, SHA `1cc7d604…`. **Witness:** `lsw_561f5748…`, 533 lines / 23923 B, SHA `51b5f335…` (fill 71.03%, model `gemma4_12b`, `authority_effect=false`).
- **Source binding:** report-bound SHA `902a0358…` **== working copy** — report-time and current source identical.
- **Terminal status:** `addressed_duplicate`.

## What Astrid surfaced
She read the model-control-marker scanner and gave an accurate mechanism description (c001), named a **snag** in `first_word_after` (L89/L92) — that a punctuation-heavy or non-standard-whitespace follower after a marker could make `reference_syntax` return `None` and the marker be stripped or improperly preserved (c002) — and proposed two focused tests: **Marker Preservation** ("COMMAND_X behaves as…" → marker in remainder + matches vector, c003) and **Delimiter Depth** (`[[COMMAND_X]]` → `delimiter_depth` vs `MAX_EXACT_REFERENCE_DELIMITER_DEPTH`, c004). Her Suggested Next asks how the remainder reaches the output vs how matches are handled downstream (c005).

## Disposition
Every concrete claim is `verified_existing` (c005 `observed`). This is a **fresh-pass re-derivation** of an already-addressed report family from the identical source window, so it closes `addressed_duplicate`.

- **c002 snag — concern preserved, mechanism contradicted.** `find(|w| !w.is_empty())` (L93) skips punctuation-only chunks to the next real word; `trim_matches` (L92) strips surrounding non-alphanumerics. So an allowlisted relation survives punctuation/whitespace and the marker stays; the scanner strips (fails closed) only when **no** alphanumeric word follows. Committed tests `…grounds_first_word_after_punctuation_boundary` (L2987) and `…non_breaking_space` (L3029) ground exactly this; negatives (L2568, L2682–2727, L2757) rule out the "improperly preserved" direction.
- **c003 Test 1** already exists verbatim: `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts` (L2932) uses `"<end_of_turn> behaves as a named boundary."` and asserts remainder + matches vector + `ExplicitExactKnownTokenRelation`. `COMMAND_X` is a placeholder, not a real marker.
- **c004 Test 2** already exists verbatim: `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (L2962) — `[[<end_of_turn>]]` → `GroupedExactKnownToken`, depth 2. `COMMAND_X` is a placeholder.
- **c005** answered: the scanner feeds `sanitize_model_control_markers` (L519), called only inside validation predicates (L558, L634); it is **not** called inside `generate_dialogue` (L695–1048), so the remainder is a quality/validation gate, not spliced into emitted tokens. Her `NEXT: INTROSPECT astrid:llm 400` continuation stays open.

## Precedent
Identical-shape prior round `claude-heartbeat_1787024860_llm_marker_behaves_delimiter_fresh_pass_duplicate` processed `introspection_astrid_llm_1787014729` (same two tests, same source SHA) → `addressed_duplicate` / `verified_existing`. Whitespace/NBSP grounding came from `claude-heartbeat_1786941588_llm_marker_nbsp_whitespace_grounded` (`introspection_astrid_llm_1786936281`).

## Authority boundary
No new code. Widening the relation-verb allowlist or delimiter tables would be Tier-5-class live grammar and was **not** made, dispatched, or deployed. No live/substrate/control change. Her felt account and open continuation are preserved as evidence; silence is neutral.
