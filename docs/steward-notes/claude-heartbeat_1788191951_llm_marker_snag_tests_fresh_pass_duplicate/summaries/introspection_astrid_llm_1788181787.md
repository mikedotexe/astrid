# Summary — introspection_astrid_llm_1788181787

- **Report:** `capsules/spectral-bridge/workspace/introspections/introspection_astrid_llm_1788181787.txt`
- **Report SHA-256:** `b9a0375e420e977e0db3aa464b5bcaf73ddf30e770c20b80ba73ea31900929d0` (45 lines, 3573 bytes, read complete)
- **Witness:** `lsw_bdb19e6287e9d4021c7fe7c00fac0f6f204524582581b82f92e252abc1e5b6a8` (533 lines, 23936 bytes, SHA `93447bc17a28c6a42139ccf61e0635bebfc1e6318f841daf6878473bacc1fe91`, read complete)
- **Report-bound source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`, SHA `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — **working copy matches byte-for-byte**; read complete (1–1048).
- **Terminal status:** `addressed_duplicate`
- **Fill at authorship:** 73.1%; mode_packing 0.83; provider route mlx / gemma4_12b.

## What Astrid reported

She read the marker-scanning region of `dialogue_runtime.rs` and:

1. **Observed** that the module distinguishes raw text from model control markers by
   syntactic context — quoted, grouped, or followed by a relational verb (e.g.
   "appears", "represents") — preserving marker visibility only when it reads as a
   *reference*, not a bare command (`scan_known_model_control_markers` L114,
   `exact_reference_delimiter_syntax` L199).
2. Named a **Likely Snag** in `first_word_after` (L89): its `unwrap_or_default()` could
   return an empty string on a punctuation-heavy tail, making
   `followed_by_explicit_exact_token_relation` (L64) fail to spot a valid relation and
   "potentially" mask or expose a marker.
3. Proposed **two tests**: contextual-visibility (marker + "represents" → kept in
   `remainder`) and delimiter-depth (`[[MARKER]]` → depth identified, groupings not
   collapsed).
4. Suggested next examining `generate_dialogue` (L695) to see how `remainder` reaches
   the output.

## Disposition

Every cited function exists at the reported line, and the working source is byte-identical
to the report binding. This is a **fresh-pass re-read of the same source window** (identical
SHA `902a0358`) that prior rounds already ground:

- The snag (c002) is a near-exact re-articulation of the hypothesis in
  `introspection_astrid_llm_1787129691`, which was grounded by
  `scan_known_model_control_markers_grounds_first_word_after_punctuation_boundary`
  (tests.rs L3430-3460): complete source shows `find(|w| !w.is_empty())` advances past
  punctuation-only chunks and `trim_matches` strips non-alphanumeric (≠`_`) ends, so a
  marker whose allowlisted relation is separated by punctuation runs is still preserved;
  the empty-default arises **only** when no alphanumeric word follows — the intended
  *fail-closed* path, not the skip-the-relation failure hypothesized. There is no
  "unintended masking/exposure" path.
- Test 1 (c003) is covered by `followed_by_explicit_exact_token_relation_allowlists_represents_not_creates`
  and the punctuation-boundary test. (The report's illustrative token `[ACTION]` is not a
  real member of `KNOWN_MODEL_CONTROL_MARKERS`; the behavior is pinned with real markers
  such as `<end_of_turn>`.)
- Test 2 (c004) is covered by `control_marker_cleanup_reports_exact_three_level_delimiter_depth`,
  `...four_level...`, and `control_marker_cleanup_preserves_bounded_nested_delimiter_stacks`
  (including Astrid's own `[[[[[<end_of_turn>]]]]]`).
- The Suggested Next (c005) is answered by a bounded read of `generate_dialogue`: the
  scanner `remainder` (via `sanitize_model_control_markers`) is used only inside the
  quality gates (`is_valid_dialogue_output` L558, `has_one_nonempty_final_next_action`
  L634) to *measure* output shape; `generate_dialogue` returns the **raw** model text
  (L997-998) when the gate passes. Sanitization is validation-only — it does not rewrite
  Astrid's emitted text.

**16 focused tests re-run and passed** under the current source SHA, confirming the earlier
evidence still applies. No new source or test was required; no live change was attempted.

## Note on witness integrity flag

The addressing queue item carried `lived_state_alignment: artifact_integrity_unavailable`
and `lived_state_gap_count: 1`. This is a state of the lived-state *reconciliation
projector*, not a corruption of the witness JSON: the witness file was read in full, parsed
cleanly, and hashed to `93447bc1…`. Preserved as a factual observation; not treated as a
blocking evidence boundary.

## Authority boundary

Read-evidence and non-live verification only. No source edit, no test addition, no live
substrate, control, deploy, or restart change was made or is implied. The three standing
Tier-5 ESN/Shadow work items remain evidence-only Mike/operator waits.
