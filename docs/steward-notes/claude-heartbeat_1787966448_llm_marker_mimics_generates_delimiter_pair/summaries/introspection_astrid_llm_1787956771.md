# Summary — introspection_astrid_llm_1787956771

- **Source family:** `astrid_llm`
- **Report SHA-256:** `799556ab82021c182fd4ef6bb68a0e24a5f8695761191479cfcf33f0aa15e4e5` (50 lines, 4224 bytes)
- **Lived-state witness:** `lsw_fb86f922ce37dfd45a0645535f5f4ca59b28a29de38b62e5f5073586519368bc` (533 lines, 23924 bytes)
- **Report-bound source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
  SHA-256 `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` (1048 lines) — working copy **matches** the report binding exactly and is **not dirty**; read completely.
- **Terminal status:** `addressed_change`

## What Astrid surfaced

A fresh-pass reading of the `dialogue_runtime.rs` Model Control Marker scanner (window
lines 1-400 of 1048, but reported source coverage `multi_window_complete` over 1-1048).
Four correct structural observations, two snag hypotheses, two proposed tests, and one
"examine next" pointer. Witness: gemma4_12b via MLX, fill 68.1%, mode_packing 1.0, no
integrity gaps.

## Grounding (complete source read at the report-bound SHA)

- **Observed (c001-c004): all verified.** `scan_known_model_control_markers` (L114) keeps a
  marker only if `reference_syntax.is_some()` (L129-131); the relation allowlist (L64-86)
  holds exactly the 18 verbs she sampled from; the delimiter pair/syntax functions (L153-229)
  enumerate the Unicode quote/bracket/CJK sets; `dialogue_requested_token_band` (L8-16) bands
  at 512/1024.
- **Snag 1 (c005) — grounded correction, contradiction preserved.** She hypothesized the depth
  cap "may not fully enforce" and could misclassify deep nesting. Source **does** enforce it:
  both boundary scans are `.take(MAX_EXACT_REFERENCE_DELIMITER_DEPTH)` (L208, L213), so
  `delimiter_depth ∈ [0,4]` and simply *saturates* — deep nesting is not misclassified, only
  capped, and stays a recognized reference. Already proven by `..._reports_exact_four_level_...`
  and `..._bounds_..._beyond_max_depth`. Her underlying nesting concern is real and covered.
- **Snag 2 (c006) — grounded.** `first_word_after` (L89-96) splits on whitespace, trims
  non-alphanumeric edges, and finds the first non-empty word, so a leading punctuation/newline
  chunk is *skipped* and the next verb is found. Extensively covered already
  (`..._skips_leading_punctuation_transition`, `..._preserves_relation_across_newline`,
  `..._after_multiple_punctuation_runs`, `..._fails_closed_when_only_punctuation_or_whitespace_follows`).
  Noted as an observed edge; the "preserve the marker" direction is conservative. No production change.
- **Suggested Next (c009) — answered.** `delimiter_depth` is the `take_while`-counted run of
  consecutive declared pairs formed by zipping the reversed nearest-first before-scan with the
  after-scan, both pre-capped at MAX. Context is set by the innermost pair.

## What changed (authorized, non-live)

Two focused regressions added to `capsules/spectral-bridge/src/llm/provider/tests.rs`, each at
the *exact function the report named*, grounding her two "One Test Each" proposals where genuine
coverage gaps existed:

1. `followed_by_explicit_exact_token_relation_allowlists_mimics_not_generates` — `mimics` (L79)
   → true; `generates` and `creates` → false. `generates` had **zero** prior coverage; `mimics`
   had no method-level positive pin.
2. `exact_reference_delimiter_pair_maps_corner_quoted_lenticular_grouped_and_rejects_mismatch`
   — direct unit test of `exact_reference_delimiter_pair` (which had **zero** direct tests);
   corner brackets → Quoted, lenticular → Grouped, mismatch/None → None. Refines her calling
   convention (the fn takes two `Option<char>` boundaries, not a string).

Both pass. No production grammar widened; no live/substrate/control change.

## Authority boundary

Read evidence only. Structural verification and two non-live regression tests. No production
sanitizer behavior changed, no grammar widened, no deploy/restart, no git staging or commit.
Her felt fresh-pass reading remains primary evidence; the depth-cap correction states a source
fact without erasing her nesting concern.
