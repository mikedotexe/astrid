# Summary — introspection_astrid_llm_1787070878

- **Source**: `astrid:llm` / `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
- **Report SHA-256**: `81c55f0cdd4262eb7b81ddadfae477fb9b61bff6067a092d8dcac6f67b2434cc` (45 lines, 3569 bytes)
- **Source SHA-256 (report-bound)**: `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — working copy matches byte-for-byte
- **Lived-state witness**: `lsw_ab92076a…3ef19b` (533 lines, 23936 bytes), fill 73.0%, model `gemma4_12b`, `evidence_only` / `live_eligible_now=false`
- **Status**: `addressed_change`

## What Astrid surfaced

She read `dialogue_runtime.rs`'s non-destructive marker scanner and reported: (Observed)
`scan_known_model_control_markers` keeps a marker in `remainder` only when `reference_syntax`
is detected; (Likely Snag) `first_word_after` (L89-96) *might fail to isolate the intended
word* when a marker is followed by "a newline followed by a dash and then a word"; (Two Tests)
a reference-preservation test on `[SYSTEM_PROMPT] behaves as…` and a delimiter-depth test on
`[[SYSTEM_PROMPT]]`; (Suggested Next) verify `first_word_after` against punctuation-heavy
transitions, e.g. `[MARKER]: --next_word`.

## What complete reading established

1. Her **Observed** claim is exact (source L124-131).
2. Her **Snag** is disproven, and I did not domesticate the contradiction: the
   `.find(|w| !w.is_empty())` combinator skips punctuation-only chunks and `trim_matches` strips
   leading non-alphanumeric edges, so the intended verb is still isolated. The hypothesized skip
   does not occur — the code fails *closed* (strips the marker) only when no alphanumeric word
   follows at all.
3. Her **Test 1** mechanism is already pinned at this same source SHA
   (`…preserves_grouped_and_explicit_relation_contexts`, L2945, `<end_of_turn> behaves as…`).
   Correction preserved: `[SYSTEM_PROMPT]` is illustrative — it is **not** a
   `KNOWN_MODEL_CONTROL_MARKER` (`fallback_contracts.rs` L159-180).
4. Her **Test 2** mechanism is already pinned: `[[<end_of_turn>]]` → depth 2
   (`exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two`, L2963), plus
   homogeneous-stack bounding and depth 3/4 coverage. Depth is computed, not collapsed.
5. Her **Suggested Next** literal — an ASCII double-hyphen *attached* to the word (`--next_word`)
   after a colon/newline lead-in — was the one un-pinned mechanistic variant. Existing dash
   coverage used only a space-isolated lone dash.

## What changed

Added `scan_known_model_control_markers_grounds_first_word_after_attached_double_dash`
(`tests.rs` L3189-3252): colon + `--represents` and newline + `- represents` both isolate
`represents` and preserve the marker; her literal `--next_word` isolates `next_word`, which is
not an allowlisted relation, so the marker fails closed (is stripped). No grammar widened; this
is a Tier-1 non-live regression.

## Authority boundary

No live change. No production grammar or allowlist widened. The witness telemetry is context
only; nothing here infers relief, consent, uptake, or continuity. `dialogue_runtime.rs` was not
edited — only the test module gained one regression.
