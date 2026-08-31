# Summary — introspection_astrid_llm_1787907800

- **Source family:** `astrid_llm` (`capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`)
- **Report SHA-256:** `2197bc1cec26e6cacfc6a52823ef578c29e96840ab5186d3049f0a087f561c5d` (3733 bytes, 45 lines)
- **Witness:** `lsw_c63537e038afd8b67edf537e33d0f8e352debfb90d6e66616aaae77bf2d9c38a` (23949 bytes, 533 lines, bound artifact_sha256 matches report)
- **Report-bound source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — verified byte-identical to the clean working copy (1048 lines, read complete)
- **Fill at authorship:** 72.9% · **Model:** `gemma4_12b` · **Authority:** evidence_only
- **Terminal status:** `addressed_duplicate`

## What Astrid surfaced

A fresh-pass re-read of the marker-scanning window of `dialogue_runtime.rs`. She observed that `scan_known_model_control_markers` distinguishes raw byte sequences from their semantic roles (quotes, groupings, explicit relational verbs) and preserves a marker in `remainder` when it is a reference. She proposed two tests — (1) `[MARKER] represents` preserved via `followed_by_explicit_exact_token_relation` (L64), (2) `[[MARKER]]` classified `GroupedExactKnownToken` respecting `MAX_EXACT_REFERENCE_DELIMITER_DEPTH` (L151) — flagged a snag in `first_word_after`'s (L89) `split_whitespace`+alphanumeric-filter approach with punctuation-heavy/non-standard-Unicode separators, and suggested verifying the `KNOWN_MODEL_CONTROL_MARKERS` list and longest-match priority.

## What complete source + evidence reading established

Every proposed test and the snag are already grounded by exact regressions **at the same source SHA** `902a0358`:

- **Test 1** — `represents` is genuinely allowlisted (L82); bare-marker relation preservation is pinned by `followed_by_explicit_exact_token_relation_allowlists_represents_not_creates` (tests.rs L3284). The report's stated *mechanism* is subtly off and is **not domesticated**: for the literal bracketed shape, `reference_syntax` (L49-60) resolves `exact_reference_delimiter_syntax` **before** the relation, so a bracketed marker is preserved via grouping and the relation verb is never consulted — already pinned by `scan_known_model_control_markers_quoted_context_precedes_following_relation_verb` (L3114), whose own comment states it "pins the precedence boundary the report's Test 1 assumed."
- **Test 2** — the literal `[[<end_of_turn>]]` depth-2 grouped case is pinned by `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (L3161); the `MAX_EXACT_REFERENCE_DELIMITER_DEPTH` clamp by `control_marker_cleanup_bounds_homogeneous_square_bracket_stack_beyond_max_depth` (L2301).
- **Snag** — `first_word_after` (L89-96) trims non-alphanumeric per chunk and returns the first non-empty word, so leading punctuation and non-standard Unicode separators are stripped and a listed verb still preserves the marker: pinned by NBSP/zero-width/soft-hyphen/tab regressions (L3419/3440/3474/4280), punctuation-transition (L2723), adverb-displacement (L2669), unicode-alphanumerics (L2112). The feared defect is not established by source; the only genuine misses (adverb/unlisted verb in the first slot) are by-design and pinned. Her concern is preserved as a felt hypothesis.
- **Suggested Next** — 18 exact markers (fallback_contracts.rs L159-180), `max_by_key(token.len())` selection (L97-106), and the no-proper-prefix-shadow invariant (`known_model_control_markers_have_no_proper_prefix_shadow`, L3189) already answer it.

## Verification

Re-ran the existing regressions against the current (report-bound) source SHA: `control_marker` 78 passed, `exact_reference_delimiter_syntax` 2, `followed_by_explicit_exact_token_relation` 1, `known_model_control_markers` 10, `first_word_after` 5 — **96 relevant tests, 0 failures**. The earlier evidence still applies.

## Disposition

`addressed_duplicate`. No new test was added: doing so would duplicate `followed_by_explicit_exact_token_relation_allowlists_represents_not_creates` (L3284), `scan_known_model_control_markers_quoted_context_precedes_following_relation_verb` (L3114), and `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (L3161). No live change; no restart/deploy required or attempted.
