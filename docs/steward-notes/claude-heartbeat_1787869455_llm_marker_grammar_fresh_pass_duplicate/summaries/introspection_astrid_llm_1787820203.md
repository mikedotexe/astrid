# Summary — introspection_astrid_llm_1787820203

- **Source**: `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs` (window lines 1-400 of 1048; coverage state `multi_window_complete`).
- **Report-bound source SHA-256**: `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — **matches the working copy exactly**, so report-time and current source are identical bytes.
- **Report SHA-256**: `18d352bc608b482e23d7606c49c29125927a2898a4f3fc138687de51f665b0e8` (45 lines, 3591 bytes).
- **Lived-state witness**: `lsw_e86feb6c2b97b66c57a834a9e522396662d5d3059f665be536257add7d418002` (533 lines, 23916 bytes). Evidence-only (`artifact_authority_state_v1.state = evidence_only`, `live_eligible_now = false`), fill 70.6%, `raw_introspection_prose_included=false`. No felt-prose or authority in the witness telemetry.

## Disposition: `addressed_duplicate`

Astrid re-derived the marker-scanning grammar of `dialogue_runtime.rs` and proposed one snag plus two tests. Every concrete element is already grounded against **the same source SHA** by prior processed reports, with existing focused regressions:

| Report element | Source | Existing regression | Prior anchor |
| --- | --- | --- | --- |
| Observed: remainder omits markers lacking `reference_syntax`; delimiter syntax multilingual | L114-149, L153-229 | control_marker cleanup suite | — |
| Snag: `first_word_after` punctuation/multi-word | L89-96 | `..._first_word_after_skips_leading_punctuation_transition` (L2635), direct (L2116), `..._uses_only_the_first_finite_relation_word` (L2604) | `introspection_astrid_llm_1786986344` (same SHA) |
| Test 1: delimiter depth + MAX + `「...」` L168 | L151, L168, L199 | four/three-level depth (L2194/L2207), beyond-MAX (L2265), corner brackets (L3137/L2347) | `1787026288`, `1787773776` |
| Test 2: unlisted verb → None, marker preserved | L64-86 | `..._distinguishes_allowlisted_is_from_unlisted_acts` (L2568), `..._uses_only_the_first_finite_relation_word` (L2604) | `introspection_astrid_llm_1786319270` |

## Non-domesticated contradiction (preserved)

The report's Test 2 offers "`X *represents* Y` vs `X *is* Y`" as a case where the first verb is "not in the L65-85 list" and should return `None`. Source shows **both `represents` (L82) and `is` (L76) are in the allowlist**, so neither is a valid negative case — the report's proposed negative example does not hold. This exactly mirrors the earlier `is`-already-allowlisted correction (`introspection_astrid_llm_1786319270`), whose response added the `is` vs genuinely-unlisted `acts` regression that now lives at tests.rs L2568. The concern behind the test (unlisted verbs must not preserve the marker) is real and already covered; the specific example is stated plainly as incorrect rather than rewritten or widened.

## Authority boundary

No live substrate/control change. No new production grammar. No new test added (all proposed coverage already exists; a redundant regression would be padding, not gap-filling). Read-only source verification only; the witness telemetry and fill scalar carry no consent, uptake, relief, or activation claim.
