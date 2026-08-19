# Summary — introspection_astrid_llm_1787086168

- **Source of record:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
  (window lines 1–400 of 1048), report-bound SHA-256 `902a0358…` — **matches** the
  current working copy exactly (no report/source drift).
- **Witness:** `lsw_efe0ba15…` (533 lines, 23937 bytes). Telemetry-only, authority
  `evidence_only` / `witness_only`; fill 71.1% (calm, near the 68% shelf), no distress
  language. Model route `gemma4_12b` via `mlx`, one repair-parent chain.
- **Disposition:** `addressed_no_action` — every concrete claim is verified against
  complete report-bound source at the recorded SHA and already covered by named
  existing regressions in `llm/provider/tests.rs`. No new code is authorized or needed.

## What Astrid observed (all accurate)

She read the non-destructive model-control-marker scanner and correctly described:
`dialogue_requested_token_band` bands (L8–16); `followed_by_explicit_exact_token_relation`
subject-marker visibility (L64–86); `exact_reference_delimiter_pair`/`_syntax` delimiter
detection (L153–229); `scan_known_model_control_markers` linear remainder reconstruction
(L114–144). Line-number bindings all check out.

## The two "Likely Snags" — resolved by source, concerns preserved

- **Snag 1 (delimiter depth exhaustion).** She hedged that
  `MAX_EXACT_REFERENCE_DELIMITER_DEPTH` (L151) "might not yet fully implement recursive or
  nested depth validation." Source **contradicts** the feared gap:
  `exact_reference_delimiter_syntax` (L199–229) bounds the collected delimiter chars with
  `.take(MAX=4)` (L208/L213) and counts concentric matching pairs via `take_while`. Context
  is always taken from the **innermost** pair, so beyond-cap nesting cannot cause incorrect
  preservation — it only saturates the *reported* depth. This is exactly what
  `control_marker_cleanup_bounds_homogeneous_square_bracket_stack_beyond_max_depth`
  (tests.rs L2266) proves — and that test itself grounds a **sibling** report,
  `introspection_astrid_llm_1787026288`, with the same worry. Her underlying
  cap-correctness concern is retained, not domesticated.
- **Snag 2 (overlapping-token ambiguity).** `longest_exact_known_model_control_marker_at`
  (L97–112) uses maximal-munch (`max_by_key(token.len())`). Enumerating all **20**
  `KNOWN_MODEL_CONTROL_MARKERS` (fallback_contracts.rs L159) shows **no marker is a strict
  prefix of another**, so the shared-prefix obscuring she hypothesized cannot occur with the
  current set. Longest-match is exercised by
  `control_marker_cleanup_preserves_longest_overlapping_token_when_named_as_content`
  (L2493). The forward-looking concern (a *future* shared-prefix marker) is preserved.

## The two "One Test Each" proposals — already regressed

- **Test 1 (contextual visibility).** Positive (`denotes` → preserved):
  `…preserves_exact_token_relation_without_preceding_vocabulary_gate` (L2540, uses
  "denotes"). Negative (unrelated word → stripped): the
  `does_not_expand_relation_allowlist_to_*` suite (L2682–2727) + `…fails_closed_when_only_
  punctuation_or_whitespace_follows` (L2900).
- **Test 2 (delimiter recognition).** `「<end_of_turn>」` →
  `…preserves_non_ascii_matching_quote_pairs` (L2342) asserts `quoted_reference_occurrences
  == 1`. Precision note (not a rewrite): her proposed entry point
  `exact_reference_delimiter_pair` takes `(before, after)` *chars*; the string-level path is
  `exact_reference_delimiter_syntax(text, start, end)`. Both are tested.

## Suggested Next — performed

She asked to "examine `exact_reference_delimiter_syntax` beyond line 207 to confirm how
`MAX_EXACT_REFERENCE_DELIMITER_DEPTH` is enforced." I did: it is enforced (bounded via
`.take(MAX)` and measured via `take_while`). Confirmed; recorded as `observed`.

## Authority boundary

No source, test, config, live substrate, or control change was made or is implied. The
dirty `llm/provider/tests.rs` and `autonomous/runtime/tests.rs` are foreign accumulated work
and were read-only. No restart/deploy was required or attempted.
