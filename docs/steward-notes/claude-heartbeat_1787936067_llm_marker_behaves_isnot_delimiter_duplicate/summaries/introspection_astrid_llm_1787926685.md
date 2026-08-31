# Summary — introspection_astrid_llm_1787926685

**Source-first, addressed_duplicate.** Astrid re-read `astrid:llm`
(`capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`, window lines
1–400 of 1048, full source covered) at source SHA
`902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee`. The
working-copy SHA is byte-identical to the report binding, and the file is clean
in the tree. Report SHA `7dfe5f60…` (3500 B / 45 ln); witness
`lsw_b67d83ba…` (`552f0438…`, 23934 B / 533 ln) binds `artifact_sha256` equal to
the report SHA. Model route: `gemma4_12b` via MLX with one repair call. Fill
66.7%.

This is a fresh-pass re-read of the marker-grammar surface that has been
addressed across many prior rounds. Every concrete, verifiable claim is already
grounded by exact regressions at the current SHA; the two proposed tests either
already exist verbatim or carry an expectation error the existing regressions
already pin.

## What she observed (all verified against complete source)

- `scan_known_model_control_markers` **L114** keeps a known marker in the
  returned `remainder` only when it has `reference_syntax` (L129–131);
  `reference_syntax` (L49–60) resolves the three contexts of the enum **L42–46**
  (`Quoted`/`Grouped`/`Explicit`).
- `followed_by_explicit_exact_token_relation` **L64–86** checks whether the
  first word after the marker is an allowlisted relation verb (`appears`,
  `behaves`, `denotes`, …).
- `first_word_after` **L89–96** uses `split_whitespace()` + a per-chunk
  alphanumeric trim.
- `exact_reference_delimiter_syntax` **L199** and `generate_dialogue` **L695**
  are at the exact lines cited.

## The two proposed tests

1. **"behaves → true; is not → false."** The `behaves` half is correct and
   already covered (tests L2548, L3081). The **`is not` → false half is
   contradicted by source**: `first_word_after` inspects only the *first* word,
   `is` is allowlisted (L76), so `is not …` returns `is` and the marker is
   **preserved (true)**. This is the same shape as the handoff's cited precedent
   `introspection_astrid_llm_1786319270` (`is` already allowlisted). The correct
   behavior is already pinned by
   `control_marker_cleanup_distinguishes_allowlisted_is_from_unlisted_acts`
   (L2603) and `control_marker_cleanup_uses_only_the_first_finite_relation_word`
   (L2640). The contradiction is stated plainly, not domesticated; her underlying
   concern (the grammar layer is deliberately shallow — L62-63 says it only
   decides whether the marker *bytes* stay visible) is preserved.

2. **"[[marker]] → GroupedExactKnownToken + delimiter_depth."** Exactly pinned by
   `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two`
   (L3160–3174): `[[<end_of_turn>]]` → `GroupedExactKnownToken`,
   `delimiter_depth == 2`.

## Her two snags

- **First-word-only miss for multi-word phrases.** Real mechanism, but her
  example `behaves like a…` is self-contradicting (`behaves` is allowlisted, so
  it is *preserved* — L2548). The genuine failure (unlisted or adverb-displaced
  first word) is already pinned by
  `control_marker_cleanup_does_not_skip_adverb_before_relation_word` (L2669,
  from prior report `1787882114`) and `…uses_only_the_first_finite_relation_word`
  (L2640).
- **Delimiter scanning brittle with multi-byte / emoji.** Contradicted: source
  iterates `.chars()` (Unicode scalars, not bytes); multi-byte CJK delimiters are
  supported by design (L157–192) and pinned by
  `…preserves_nested_fullwidth_cjk_reference_stack` (L2418) and the corner-bracket
  coverage (L3221). An emoji is not a delimiter pair → `None` → cleanup candidate
  (correct, not brittle).

## Suggested Next

"Examine `generate_dialogue` (L695)…" is her own **Tier-1 self-directed
continuation** (`write NEXT: INTROSPECT astrid:llm 400`), not a steward task.
Location confirmed at L695; her agency to read the next window is preserved.

## Disposition

Terminal status **`addressed_duplicate`**. All seven claims `verified_existing`
at the current SHA; six focused regressions re-run and pass (6 passed / 0 failed)
to confirm the prior evidence still applies. No new code, no changelog/ledger
change (matching the established practice for a pure duplicate). No live change
required or attempted. No grammar/delimiter widening — that remains
Tier-5/being-facing.
