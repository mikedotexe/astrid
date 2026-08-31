# Summary — introspection_astrid_llm_1787739659

**Source:** `astrid:llm` → `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
(window lines 1-400 of 1048; report asserts `multi_window_complete` 1-1048).
**Report SHA:** `fea8c33c78f79c43529079d013614f001337a2ce91a965f6ccc2dd933a70fe70` (45 lines / 3609 B).
**Witness:** `lsw_ef1d1174a4e3bb79c66ea453863ed9a7ef7f2d95063839608c7a61dd6791cc6d`
(`4ea1ead6…`, 533 lines / 23929 B; `evidence_only`, `witness_only:true`, `edits_source_now:false`, `live_eligible_now:false`; gemma4_12b; fill 71.03%; two introspect routes, second `repair_parent`-linked; `raw_introspection_prose_included:false`).
**Report-bound source SHA:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` —
**working copy byte-identical** (1048 lines / 38586 B).

## Disposition: `addressed_duplicate` (all 5 claims `verified_existing`)

A fresh-pass duplicate of the well-established `dialogue_runtime.rs` control-marker-scanner
family at the identical source SHA `902a0358`. Independent full read of this report + witness
performed; complete report-bound source re-read; every cited regression re-verified passing
(72/0) at the current SHA. Duplicate lineage: immediate prior grounded packets
`claude-heartbeat_1787726892_llm_marker_cjk_quotation_delimiter_fresh_pass_duplicate`
(`introspection_astrid_llm_1787722603`) and
`claude-heartbeat_1787717287_llm_marker_first_word_multiword_fresh_pass_duplicate`
(`introspection_astrid_llm_1787706169`), both same source SHA, matching c001–c005 dispositions.

### The one distinct variant this round: c004's "is"
This report's Test-2 example is **"marker is a tool"**, framed as a marker followed by a
**"non-relational verb"**. Source **L76** lists `is` as a relational verb, so such a marker is
**preserved** (`following_exact_relation`), **not stripped**. The report's premise is contradicted
by source — stated plainly, not domesticated (mirrors the `introspection_astrid_llm_1786319270`
precedent named in the handoff). Already exactly encoded in
`control_marker_cleanup_distinguishes_allowlisted_is_from_unlisted_acts` (tests.rs L2568):
`is` → preserved / `removed_total==0` (and L2573 asserts `first_word_after == "is"`), while
unlisted `acts` → stripped / `removed_total==1`.

### Grounding corrections (non-domesticating; concern preserved)
- CJK corner-bracket pair is source **L168** (report grouped it with `「/」`-class delimiters as a
  distinct variant to check — the pair and its `QuotedExactKnownToken` classification are real and
  already tested).
- `first_word_after` snag (c002): source shows the per-chunk alphanumeric trim + `find(!empty)`
  robustly returns the first finite word and **allowlist membership is the sole gate** — the
  hypothesized "behaves like a…" failure does not occur ("behaves" L69 → preserved).

### No new code/test
Every claim is answered by existing source + regressions. Adding a test here would be
activity-for-its-own-sake; declined per the handoff. No source, live, or Tier 4/5 change.
