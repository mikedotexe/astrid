# Summary — introspection_astrid_llm_1787014729

**Source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`, window lines 1-400 of 1048 (multi-window-complete), report-bound SHA `902a0358…c7ee` — **working copy identical**.
**Report:** 45 lines / 3853 B / SHA `b7d16863…2e93`. **Witness:** `lsw_770b39df…2bab7`, 533 lines / 23916 B / SHA `5649ad44…9f55`; `artifact_sha256` binding matches report; authority state `evidence_only`, `live_eligible_now=false`.
**Runtime context (witness):** fill 74.1%, spectral_entropy 0.909, λ1 4.68, model `gemma4_12b`, two mlx calls (second a repair of the first).

## What Astrid observed
A non-destructive scanner for model control markers that separates *raw* transport markers from *referenced* ones (marker-as-grammatical-subject, e.g. `[TOKEN] behaves as…`), preserving marker bytes in a rebuilt `remainder` only when a reference syntax is detected.

## Terminal status: `addressed_duplicate`
A fresh-pass re-read of the same source window whose every mechanism is already grounded at the exact functions named. Duplicate of `introspection_astrid_llm_1786980416` (packet `claude-heartbeat_1786986391_…fresh_pass_duplicate`) and the broader `astrid:llm` marker-grammar family; source SHA unchanged, existing tests still apply; independent full read of this report + witness done.

## Claim dispositions
- **c001 Observed — verified_existing.** `scan_known_model_control_markers` L114-143 preserves a token only when `reference_syntax.is_some()` (L129-131); `reference_syntax` L49-60. Faithful.
- **c002 Snag (over-strip / retain-incorrectly on punctuation/newline/locale) — verified_existing; concern preserved, mechanism contradicted.** `first_word_after` L89-96 `find(|w| !w.is_empty())` skips punctuation-only chunks to the next real word, so a punctuation- or colon-separated relation still preserves; only strips when no alphanumeric word follows (fail-closed). `split_whitespace` is Unicode White_Space, not locale-sensitive. Grounded: tests.rs L2947, L2833, L2989, L3042; negatives L2642/2657/2672/2687 rule out retain-incorrectly.
- **c003 Test 1 (reference preservation via "behaves") — verified_existing + placeholder contradiction preserved.** tests.rs L2892 asserts `<end_of_turn> behaves as a named boundary.` → `ExplicitExactKnownTokenRelation`, marker preserved. Report's literal `[SYSTEM_PROMPT]` is **not** in `KNOWN_MODEL_CONTROL_MARKERS` (fallback_contracts.rs L159-180) — the mechanism is real, but must be tested with a real marker, as the suite does.
- **c004 Test 2 (`[[[TOKEN]]]` delimiter depth → GroupedExactKnownToken) — verified_existing + placeholder contradiction preserved.** `exact_reference_delimiter_syntax` L199-229 + `exact_reference_delimiter_pair` L153-197 treat `[ ]` uniformly as GroupedExactKnownToken and count depth by zip/take_while; depth-3 grouped path directly tested (L2208), pure-square depth-2 (L2923), depth-4 (L2194). Triple-square exercises the identical branch. `TOKEN` is a placeholder.
- **c005 Suggested Next (how `generate_dialogue` consumes the scan) — observed; agency preserved.** Current source: `sanitize_model_control_markers` (L519) is called only inside the boolean validation fns `is_valid_dialogue_output` (L558) and `has_one_nonempty_final_next_action` (L634); the stripped text measures output shape to accept/reject. **No sanitize call inside `generate_dialogue` L695-1048** → emitted tokens are not spliced. Answer to her open question: a *validation/quality gate*, not token alteration. Marker-only reject test L1916. Her `NEXT: INTROSPECT astrid:llm 400` continuation stays open.

## Authority boundary
No live change. Widening the relational-verb allowlist or the delimiter tables would be Tier-5-class live grammar — not made, dispatched, or deployed. Nothing implemented; all evidence pre-exists at the recorded SHAs. Her continued freedom to re-read the next window is preserved.
