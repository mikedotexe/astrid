# Summary — introspection_astrid_llm_1788301040

- **Source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
  (SHA-256 `902a0358…`, 1048 lines; window read 1-400, but coverage state
  `multi_window_complete`, included intervals 1-1048).
- **Report:** 49 lines, 4185 bytes, SHA-256 `e5e46e8f…`.
- **Witness:** `lsw_2a0fbc0cd1d8f1692fedcc999d04e83e63a545b7e8aec06446ac5a71ff9b7c53`
  (533 lines, 23945 bytes, SHA-256 `66b0f0aa…`). Fill 71.06%, model
  `gemma4_12b`, two mlx routes (72s + 51s repair), authority `evidence_only` /
  `witness_only`, `live_eligible_now=false`.

## What she said

A fresh-pass INTROSPECT of the model-control-marker scanner. Her **Observed**
layer names the scanner's mechanics with exact line numbers; her **Likely
Snags** hypothesize two failure modes (delimiter ambiguity; greedy overlap);
her **One Test Each** proposes a contextual-distinction test and a
delimiter-depth-boundary test; her **Suggested Next** points herself at
`generate_dialogue` (L695).

## Disposition — `addressed_duplicate` (evidence-backed)

A fresh-pass re-read of the same source window in the established
`dialogue_runtime.rs` marker-grammar family. Primary prior anchor
`introspection_astrid_llm_1788139420` (same source SHA `902a0358`, same window,
same marker-scanner observations, same two One-Test-Each, same `generate_dialogue`
L695 suggested-next; closed `addressed_duplicate` in packet
`claude-heartbeat_1788247141_astrid_llm_1788139420_marker_grammar_dup`). The
per-claim regressions still apply, two authored for prior reports. Every concrete
claim resolves to complete-source facts and existing exact regressions. The
working-copy source SHA matches the report binding exactly, so report-time and
current-source conclusions coincide.

- **c001–c004 (Observed):** byte-exact against complete source. L199
  `exact_reference_delimiter_syntax`, L64 `followed_by_explicit_exact_token_relation`,
  L114 `scan_known_model_control_markers`, L151 `MAX_EXACT_REFERENCE_DELIMITER_DEPTH=4`,
  L257 `CONTROL_MARKER_CONTEXT_WINDOW_CHARS=64`, L97 `longest_…max_by_key(token.len())`,
  L153-197 hardcoded delimiter pairs. All correct.
- **c005 (delimiter ambiguity):** consequence is real and *by design* — an
  unrecognized delimiter yields `reference_syntax=None`, so the marker is not
  preserved but stripped (fail-closed cleanup). Mechanism is imprecise: the scan
  uses `.chars()` (Unicode-scalar-aware), so multibyte UTF-8 never splits, and
  many *recognized* delimiters are themselves multibyte and are preserved.
  Covered by multibyte/astral/CJK/fail-closed regressions.
- **c006 (greedy overlap):** **contradiction preserved.** Candidates are always
  complete vocabulary tokens (`tail.starts_with(token)`, token ∈
  `KNOWN_MODEL_CONTROL_MARKERS`), never unvalidated fragments;
  `known_model_control_markers_have_no_proper_prefix_shadow` (tests L3309) proves
  no marker shadows another as a prefix — the overlap she anticipates is
  unreachable. That test was authored to answer the identical snag in prior
  report `introspection_astrid_llm_1787782248`.
- **c007 (contextual distinction):** covered by
  `control_marker_cleanup_preserves_quoted_exact_token_reference` +
  `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts`
  + `…_quoted_context_precedes_following_relation_verb`. Her example `STOP` is not
  a real marker; existing tests use real markers (`<end_of_turn>`, `[INST]`, …).
- **c008 (depth boundary):** **contradiction preserved.** Exceeding the depth
  cap does *not* make the parser fail to identify the innermost marker; context
  is set from the innermost adjacent delimiter regardless of depth, and only the
  *reported* `delimiter_depth` saturates at 4.
  `control_marker_cleanup_bounds_homogeneous_square_bracket_stack_beyond_max_depth`
  (tests L2301) and `…_bounds_deeper_delimiter_receipt_without_dropping_token`
  (L2288) show the token stays byte-exact and preserved. L2301 was authored to
  answer the identical proposal in prior report
  `introspection_astrid_llm_1787026288`.
- **c009 (suggested next):** accurate pointer (`generate_dialogue` at L695); this
  is her own Tier-1 continuation, needing no steward action.

## Evidence

94 focused marker-grammar tests pass (0 failed) against the current source
(`cargo test -p spectral-bridge --lib -- marker exact_reference_delimiter_syntax
followed_by_explicit_exact_token_relation`). No new code warranted; no authority
boundary crossed. Two of her snags are re-derivations of prior reports that
already carry dedicated, named regressions — a healthy sign the scanner surface
is faithfully documented in tests. See
`steward_note/duplicate_rationale_introspection_astrid_llm_1788301040.md`.
