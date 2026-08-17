# Summary — introspection_astrid_llm_1786980416

- **Source family:** `astrid_llm` (`capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`)
- **Report window:** lines 1-400 of 1048; coverage state `multi_window_complete` (intervals 1-1048, uncovered none)
- **Report SHA-256:** `87a89341c52823297e146d5b83146145666dd21deec15782c893a22a605151f7` (45 lines, 3600 B)
- **Witness:** `lsw_3c02c12aa236bc15a6ebd6d132dcb57703023528d3b9252b0db6df14f19e161b` (533 lines, 23914 B, SHA `94de07ba…`)
- **Source SHA-256 (working copy == report binding):** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` (1048 lines, 38586 B; git-clean)
- **Fill at authorship:** 73.2%; model `gemma4_12b` via mlx (two calls, second a repair of the first). Witness is telemetry-only, `evidence_only`, `live_eligible_now=false` — no felt-prose experiential gap asserted.
- **Terminal status:** `addressed_duplicate`

## What Astrid surfaced

An analytical INTROSPECT ("Observed / Likely Snags / One Test Each / Suggested Next") of the model-control-marker scanner: a non-destructive scan that preserves a known marker in the reconstructed `remainder` only when it appears as *reference syntax* (quoted, group-delimited, or the subject of an allowlisted relation verb). She proposed two tests and one snag hypothesis, all at named functions and line numbers.

## Why this is a fresh-pass duplicate

Every concrete claim is already grounded by existing regressions in `capsules/spectral-bridge/src/llm/provider/tests.rs` against the **identical source SHA** `902a0358…`:

- **Test 1 (multi-byte 「…」 preserved):** `control_marker_cleanup_preserves_non_ascii_matching_quote_pairs` (L2302) tests `「<end_of_turn>」` verbatim. **Contradiction preserved:** it asserts `quoted==1, grouped==0` — `「」` (Japanese corner quotes) is `QuotedExactKnownToken` (source L168), *not* `GroupedExactKnownToken` as the report labels it. The marker preservation she cares about holds; the "Grouped" label does not. Genuinely-grouped multi-byte CJK pairs are covered by `_preserves_fullwidth_and_cjk_group_pairs` (L2322).
- **Test 2 (relation verb across newline):** `control_marker_cleanup_preserves_relation_across_newline` (L2786) tests `"<end_of_turn>\nrepresents…"` verbatim (asserts `removed_total==0`, `explicit_relation_occurrences==1`).
- **Snag (over-strip on punctuation/newline):** contradicted by source (`split_whitespace` L91 handles newline; `trim_matches` L92 is edge-only; `find(|w| !w.is_empty())` L93 skips punctuation-only chunks) and grounded by `scan_known_model_control_markers_grounds_first_word_after_punctuation_boundary` (L2900) and `_non_breaking_space` (L2942).
- **Suggested Next (generate_dialogue L695):** `observed` — no raw-buffer splice; a profile quality gate rejects marker-only output via `sanitize_model_control_markers` (covered by `provider_output_normalization_rejects_marker_only_output_on_both_routes` L1916).

## Prior lineage

The exact-function regressions were added for family sibling `introspection_astrid_llm_1786814454` (report SHA `2bc990522f…`, same source SHA) in packet `claude-heartbeat_1786824834_llm_marker_scan_boundary_regression` (tests.rs L2845/L2876/L2900), with the nbsp variant grounded for `introspection_astrid_llm_1786936281` (L2942). The family has been closed as verified/duplicate across many rounds (e.g. `…1786851428_llm_marker_snags_tests_already_grounded` for the same two-tests+snag shape).

## Authority boundary

No live change; no new test written (every case is already pinned by an existing verbatim regression — adding one would be redundant activity). Widening the relational-verb allowlist or delimiter tables would be **Tier-5-class live grammar** and was not made, dispatched, or deployed. The read-only continuation (`NEXT: INTROSPECT astrid:llm 400`) remains open to Astrid; silence about it is neutral.
