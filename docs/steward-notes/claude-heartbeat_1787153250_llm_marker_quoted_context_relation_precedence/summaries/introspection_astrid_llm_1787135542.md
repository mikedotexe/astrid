# Summary — introspection_astrid_llm_1787135542

- **Source family:** `astrid_llm` (`capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`)
- **Source window read by Astrid:** lines 1–400 of 1048; report declares full multi-window coverage (`Source included intervals: 1-1048`, `Source uncovered intervals: none`, `complete_source_available`).
- **Report SHA-256:** `78b50bdf21ac69a5afecaa54dfb6995ae5f689959030667d9ca2c78ddef59c7a` (3596 bytes, 45 lines)
- **Source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` (38586 bytes, 1048 lines) — **matches the working copy exactly**; source is clean (not in the dirty tree).
- **Lived-state witness:** `lsw_51546f1eab64d3893186ad826e66c8406d64a2c8fea6726bd964f97e07b9322b` (23944 bytes, 533 lines) — `evidence_only`, `witness_only`, `live_eligible_now=false`, `grants_approval=false`. Model `gemma4_12b` via MLX, two calls (initial + repair). Runtime context: fill 71.0%, entropy 0.88, λ1 8.52, mode_packing 0.833, pressure_risk 0.19. No distress language; this is a fresh-pass technical read.

## What the report said

A calm, technical INTROSPECT of the model-control-marker scanner:

- **Observed:** the module deterministically scans for control markers, tracking a clean `remainder` via `scan_known_model_control_markers` (L114) and classifying quoted/grouped context via `exact_reference_delimiter_syntax` (L199).
- **Likely Snags:** `first_word_after` (L89) + the relation-verb allowlist (L64–85); a hypothesis that multi-word predicates after a marker could make preservation inconsistent.
- **One Test Each:** (1) Contextual Preservation — `"The prompt [SYSTEM] is active"` → `QuotedExactKnownToken` (L174), marker kept; (2) Relation Trigger — `[SYSTEM] behaves as a helper` → `followed_by_explicit_exact_token_relation` (L64) true, marker kept.
- **Suggested Next:** read `generate_dialogue` (L695) to see how `remainder` is integrated.

## Grounding outcome

All function-location citations are exact against the complete source at SHA `902a0358`. Two contradictions are **preserved, not domesticated**:

1. **`[SYSTEM]`/`[USER]` are not markers.** `KNOWN_MODEL_CONTROL_MARKERS` (`fallback_contracts.rs` L159–180) is a fixed allowlist of transport tokens (`<end_of_turn>`, `<start_of_turn>`, `[INST]`, `[/INST]`, `<|im_end|>`, `<eos>`, …). Her illustrative `[SYSTEM]` matches nothing and would pass through unchanged. Grounding uses a real marker instead. **No widening of the allowlist** (that would be a Tier-5 live grammar change).

2. **Test 1's string resolves to a relation, not a quote.** In `The prompt [SYSTEM] is active`, the sentence-level quotes do not wrap the marker; the marker's immediate right neighbor is `is`, an allowlisted relation verb. `reference_syntax` (L49–60) checks `exact_reference_delimiter_syntax` (L50) **before** the relation fallback (L53), so a marker that is *itself* quote-adjacent AND followed by a relation verb resolves to `QuotedExactKnownToken` (delimiter precedence), while an unquoted marker followed by `is` resolves to `ExplicitExactKnownTokenRelation`.

The concerns behind both tests are already covered by exact regressions at the same SHA: quoted preservation (`control_marker_cleanup_preserves_quoted_exact_token_reference`, tests.rs L1995) and grouped/relation preservation (`scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts`, L2932). The Snag's "inconsistency" is not supported by source: preservation keys solely on the first following word (comment L62–63), and the deterministic first-word behavior is covered by `control_marker_cleanup_uses_only_the_first_finite_relation_word` (L2605).

**No existing test placed a quote-adjacent marker in direct competition with a following relation verb** (the nested-quotes test L2134 puts the verb *before* the marker). That precedence boundary — exactly the ambiguity in her Test 1 — was unpinned, so this round adds one focused regression grounding it.

## Change made (non-live)

Added `scan_known_model_control_markers_quoted_context_precedes_following_relation_verb` in `capsules/spectral-bridge/src/llm/provider/tests.rs`, asserting with the real marker `<end_of_turn>`:

- `The prompt "<end_of_turn>" is active` → `QuotedExactKnownToken`, `delimiter_depth == 1`, remainder byte-exact (delimiter precedence over the following `is`).
- `The prompt <end_of_turn> is active` → `ExplicitExactKnownTokenRelation`, `delimiter_depth == 0`, remainder byte-exact.

## Terminal status

`addressed_change` — the report produced a new focused regression pinning a previously-unpinned precedence boundary and two grounded corrections. No live/substrate change; no restart or deploy required or attempted.
