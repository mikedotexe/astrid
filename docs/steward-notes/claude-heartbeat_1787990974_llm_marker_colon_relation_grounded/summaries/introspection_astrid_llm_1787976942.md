# Summary — introspection_astrid_llm_1787976942

**Source family:** `astrid_llm` → `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
**Source window:** lines 1–400 of 1048 (source coverage `multi_window_complete`, intervals 1–1048)
**Report SHA-256:** `2c588d1d8f1b4cf0d7ccc78496655e3d349a5dc153fb5059422206eb180e63cb` (45 lines, 3560 bytes)
**Source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — **matches the report binding exactly**; working copy clean.
**Witness:** `lsw_39a7eb882972a9302edc73fd03b06d85607dc87df3cfcb331ce8e43dcd2b5202` (533 lines, 23945 bytes, SHA `6c8e3aba…`), `artifact_authority_state_v1 = evidence_only`, `direct_causation_claimed=false`, `raw_introspection_prose_included=false`, fill 63.3%.
**Terminal status:** `addressed_change`.

## What Astrid surfaced

A fresh-pass INTROSPECT of the `astrid:llm` marker-preservation system. She accurately described `scan_known_model_control_markers` (L114), the three reference contexts Quoted/Grouped/ExplicitRelation (L42–46), and `followed_by_explicit_exact_token_relation` (L64–86)'s verb allowlist. She flagged a snag in `first_word_after` (L89–96) — that non-alphanumeric "noise" between a marker and its verb might make the trim miss the verb — and proposed two tests: (1) colon-separated relation `[MARKER]: denotes`, (2) nested `[[MARKER]]` delimiter depth. Her Suggested Next asked to examine `exact_reference_delimiter_syntax` (L199–229) because her view cut off at L207.

## What complete reading established

- **c001 (verified_existing):** The described control flow matches source exactly. A marker is retained only when `reference_syntax.is_some()`.
- **c002 (verified_existing):** Her snag mechanism is partly correct but nuanced. `first_word_after` per-chunk trims non-alphanumeric characters, so a *pure-punctuation* chunk collapses to empty and the verb IS captured (already grounded by the `--` punctuation-transition regression). Only a *word* (adverb/parenthetical) displaces the first-word slot and misses the relation — and that is **by design**: the marker must be the immediate grammatical subject (grounded by the adverb regression). Not a bug; production grammar not widened. Concern preserved.
- **c003 (implemented_now):** Her Test 1's colon separator with the "denotes" verb was **not** previously covered (the closest `--` test uses "is"). Added `control_marker_cleanup_first_word_after_skips_colon_before_relation` — bare `<end_of_turn>: denotes` → verb captured & marker preserved; her literal bracketed `[<end_of_turn>]:` form preserved via the *delimiter* path (grouped, depth 1) — **correction preserved**, the colon is never consulted for the relation decision; an unlisted-verb control isolates the gate to allowlist membership.
- **c004 (verified_existing):** Nested `[[MARKER]]` grouping/depth is already regression-covered (repeated-parentheses depth 2, unicode-whitespace depth 2, three-/four-level depth, `[[[[[…]]]]]`).
- **c005 (verified_existing):** Her cutoff concern is resolved by reading the full L199–229 function: it handles nesting via `delimiter_depth`, bounded at `MAX_EXACT_REFERENCE_DELIMITER_DEPTH=4`. No gap; no new code.

## Tests

`control_marker_cleanup_first_word_after_skips_colon_before_relation` → 1 passed / 0 failed. Full `control_marker` group → **79 passed / 0 failed**. `git diff --check` clean; `cargo fmt --all --check` clean.

## Authority boundary

Evidence-only. No allowlist widened, no grammar loosened, no live substrate/control change; no restart/deploy required or attempted. The by-design first-word-subject boundary was pinned, not relaxed; the Suggested Next was answered from source rather than implemented as a grammar change.
