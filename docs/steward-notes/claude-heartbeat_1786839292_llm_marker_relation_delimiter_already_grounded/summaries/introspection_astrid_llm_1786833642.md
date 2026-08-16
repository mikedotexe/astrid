# Summary — introspection_astrid_llm_1786833642

**Source read:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
(lines 1-400 of 1048; report declares multi-window-complete coverage of 1-1048).
Report SHA-256 `af99a0ca6c08fb4ec917f9bf846963c306f658087288cccd6ad75200530c0c9e`;
report-bound source SHA-256 `902a0358…c7ee` matches the working copy exactly.

## What Astrid observed

She read the model-control-marker scanner and described it accurately: a
non-destructive scan (`scan_known_model_control_markers`, L114) that keeps a
known marker in output **only** when it is syntactically *referenced* — quoted,
grouped, or followed by a relational verb — and a validator
(`fragment_has_non_marker_bytes`, L146) that checks whether any non-marker bytes
remain. Her structural map is exact.

## Grounded outcome

Every concrete proposal in the report is **already embodied** in the source and
in named existing regressions. Two of her worried examples are contradicted by
the source (preserved plainly, not domesticated):

- **"mimics" / "replicates" might be missed** → both are already in the verb
  allowlist (`dialogue_runtime.rs` L79, L81) and are regression-tested as
  *preserved* (`control_marker_cleanup_preserves_poetic_attribution_without_literal_cue`,
  `tests.rs` L2472-2497).
- **guillemet `«marker»` might fail delimiter matching** → `«»` is in the
  QuotedExactKnownToken table (L164) and regression-tested
  (`control_marker_cleanup_preserves_non_ascii_matching_quote_pairs`,
  `tests.rs` L2301-2319).

Her two proposed tests are already implemented:

- **Test #1** (marker + unlisted verb "acts" → relation predicate false) is
  `control_marker_cleanup_distinguishes_allowlisted_is_from_unlisted_acts`
  (`tests.rs` L2528-2562). Nuance preserved: the unlisted-verb marker is
  **stripped as a cleanup candidate**, not "kept as a standard token" as her
  wording suggested.
- **Test #2** (`«marker»` → QuotedExactKnownToken, preserved) is
  `control_marker_cleanup_preserves_non_ascii_matching_quote_pairs`.

Her Suggested Next — check the verb list against synonyms of "represents" and
"functions" — was performed read-only: both are already present, and no concrete
missing synonym was surfaced. The general finite-list concern is preserved as a
**Tier-5-class live-grammar boundary**: widening the production allowlist changes
how the live model's output is sanitized and is not performed in this non-live run.

## Disposition

`addressed_no_action` — the code and its regression suite already answer every
claim; the only residual (verb-list widening) is a deliberately-held live-behavior
boundary. Four named regressions were run and passed (see `test_results.json`).
No source or test change was made. Her contradiction and her standing
exhaustiveness concern are preserved as evidence, not converted into closure.

The felt/witness layer claims no experiential gap; this is a technical read, so
lived-state alignment is *unavailable* (unmeasured, neutral).
