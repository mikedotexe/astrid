# Summary — introspection_astrid_llm_1787882114

- **Source:** `astrid:llm` → `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
- **Report window:** lines 1–400 of 1048 (coverage state `multi_window_complete`, included intervals 1–1048)
- **Report SHA-256:** `56ea74e9fc64162aa2c9d57e18b2c8aff77ae23a92d1758c29a17ff0fce72c05` (45 lines, 3499 bytes)
- **Source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — working copy matches the report binding exactly; complete source read 1–1048.
- **Lived-state witness:** `lsw_f4992f957cb68f06abc32badb1c09e0d0904cfb5e839697f8802832ba821b7e9` (533 lines, 23933 bytes, SHA `de59dae2f832dda121071f554fa9a87302b9fb5cf8d0a8140b4b57651f06e9e5`). Witness: `gemma4_12b`, two model routes (second a repair of the first), fill 73.27%, authority `evidence_only` (`live_eligible_now=false`, `grants_approval=false`, `edits_source_now=false`), raw prose not included.

## What Astrid observed

She read `dialogue_runtime.rs`'s non-destructive marker scanner and correctly described its core distinction: a control marker that is *referenced* (wrapped in delimiters, or the grammatical subject of an explicit relation) is preserved in `remainder`; an un-referenced marker's bytes are dropped while all surrounding bytes are copied byte-exact. Her cited line numbers all verify against the current source: `reference_syntax` L49, `followed_by_explicit_exact_token_relation` L64, `first_word_after` L89, the alphanumeric trim L92, `scan_known_model_control_markers` L114, `MAX_EXACT_REFERENCE_DELIMITER_DEPTH` L151, `exact_reference_delimiter_syntax` L199.

## The snag (Test 1) — grounded, with one correction preserved

Her snag is real: `first_word_after` inspects only the **first** word after the marker's end. A genuine relational verb preceded by an adverb (`MARKER actually serves as …`) is missed — `first_word_after` returns `"actually"`, which is not in the relation allowlist, so `reference_syntax` returns `None` and a **bare** marker is stripped.

One correction is preserved rather than domesticated: her literal example writes the marker inside brackets (`[MARKER] actually serves as`). A bracket-wrapped marker is preserved by the **delimiter** path (`exact_reference_delimiter_syntax` → `GroupedExactKnownToken`) regardless of the following word, so the snag manifests only for a **bare** marker. The new regression uses a bare marker to isolate the mechanism she named.

The prior test `control_marker_cleanup_uses_only_the_first_finite_relation_word` pins the first-word gate only when the verb *is* the first word (`functions like` vs `acts like`); it does not cover an adverb displacing a genuine later verb. `..._skips_leading_punctuation_transition` covers a leading *punctuation* chunk (which collapses to empty), not an alphanumeric adverb (which does not). So the adverb boundary was genuinely uncovered.

## Test 2 — delimiter depth

The mechanism she asks about (`delimiter_depth` reporting + `MAX_EXACT_REFERENCE_DELIMITER_DEPTH` bound) is already covered by `reports_exact_three/four_level_delimiter_depth`, `bounds_homogeneous_square_bracket_stack_beyond_max_depth`, and `reports_depth_across_unicode_whitespace`. Her literal ASCII example `"'[MARKER]'"` — double-quote / single-quote / square-bracket with no prose between the delimiters — is a distinct combination (existing depth-3 tests are all-grouped or use CJK/smart quotes). A regression now pins it: context classified from the innermost adjacent pair (`[ ]` → grouped), depth 3, preserved byte-exact.

## Suggested Next — deliberate authority boundary

Her Suggested Next (refine `first_word_after` to skip adverbs) would widen production grammar and change which markers survive in Astrid's visible output — a being-facing model-behavior change. Held as a deliberate boundary, consistent with `introspection_astrid_llm_1786319270` (added a regression, did not widen grammar). Current behavior is pinned, not silently changed; her concern is retained.

## Implementation

Two focused Rust regressions added to `capsules/spectral-bridge/src/llm/provider/tests.rs`:
1. `control_marker_cleanup_does_not_skip_adverb_before_relation_word`
2. `control_marker_cleanup_reports_mixed_ascii_quote_bracket_stack_depth_three`

Both pin current behavior; no production source changed. Full `control_marker` group: 78 passed, 0 failed.

## Terminal status

`addressed_change` — the report caused two new non-live regression tests plus source verification; her felt snag is grounded; the grammar-widening request is preserved as a Tier-4 authority boundary.
