# Summary — introspection_astrid_llm_1788115759

**Source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
(window 1-400 of 1048; coverage manifest multi_window_complete for 1-1048)
**Report-bound source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee`
**Current source SHA-256:** identical — complete source read (L1-1048).
**Report SHA-256:** `804e21a2ab6ebce37cda9991e72fce7c2ebcf43653e82afacf91be8071076adc` (50 lines, 4174 bytes)
**Witness:** `lsw_aea737ea963b4e7f64f043d96ac361dccb9329989fc14d39e577430ffc89cb64` (533 lines, 23923 bytes, SHA `bd13c957...`; artifact_sha256 matches report; fill 71.1%, model gemma4_12b/mlx).

## What Astrid surfaced
A fresh-pass introspection of the "Model Control Marker" scanning grammar in
`dialogue_runtime.rs`: the non-destructive scanner, contextual reference
recognition, preservation logic, and international-delimiter support. She raised
two snags (over-aggressive sanitization exposing internal tokens; delimiter
depth beyond `MAX=4` failing to classify context) and proposed two tests: a
**"behaves"** relation-verb visibility test, and a **five-level
`[[[[[MARKER]]]]]`** delimiter-exhaustion test.

## What complete source + tests established
- All descriptive citations are accurate at the exact SHA (functions at
  L114/L49/L146/L199, verb allowlist L64-86, delimiter pairs L157-192).
- **Both proposed tests already exist as named, passing regressions** at this
  same source SHA:
  - "behaves" → `control_marker_cleanup_preserves_poetic_attribution_without_literal_cue`
    (tests.rs L2547; `"behaves as"`/`"behaves like"` L2558-2559). "behaves" is
    allowlisted at dialogue_runtime L69.
  - depth-5 `[[[[[<end_of_turn>]]]]]` →
    `control_marker_cleanup_bounds_homogeneous_square_bracket_stack_beyond_max_depth`
    (tests.rs L2315). Its comment (L2302) records that a **prior introspection,
    `introspection_astrid_llm_1787026288`, drove this exact case.**
- Both snags' feared failures do **not** occur: preservation is gated on
  `reference_syntax.is_some()` with a deliberately narrow, guard-tested verb
  allowlist; depth-5 still classifies `GroupedExactKnownToken` from the
  innermost pair, clamps reported depth to 4, and never drops the token.
- 87 marker-grammar tests pass, 0 fail, at the current SHA.

## Disposition
Every concrete claim is `verified_existing`. The report proposes the same two
mechanisms already grounded by named regressions at the identical source SHA;
no new code, contradiction-repair, or authority decision is warranted.

**Terminal status: `addressed_duplicate`.** The contradiction in each snag is
stated plainly, not domesticated; her boundary concerns are preserved as
evidence even though the current design already handles both cases.
