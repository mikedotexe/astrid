# Summary — introspection_astrid_llm_1787789890

- **Source family:** `astrid_llm` → `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
- **Report SHA-256:** `99a25fb7f8bcf9ef54048db3987e5d98693e0f400fa3ee9fa892b32cbef38e94` (45 lines, 3755 B, read complete)
- **Witness:** `lsw_e980812819016abe0d3457705624af46004d7b801cf9520ab2fcb1b61e0e0852` (533 lines, 23942 B) — `evidence_only`, `live_eligible_now=false`, fill 72.17%, gemma4_12b/mlx
- **Source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` == report binding; working tree clean
- **Terminal status:** `addressed_duplicate`

## What Astrid surfaced

A fresh-pass reading of the `dialogue_runtime.rs` model-control-marker scanner (lines 1–400).
She observed the mechanism — markers are kept in output only when they carry "reference syntax"
(quoted / grouped / followed by a relation verb), orchestrated by `scan_known_model_control_markers`
(L114). She raised one **Likely Snag** about `first_word_after` (L89) potentially failing verb
extraction when a complex punctuation mark or a Unicode whitespace variant is not caught by the
`!c.is_alphanumeric() && c != '_'` trim filter. She proposed **Two Tests**: (1) Relation Persistence
for a verb separated from the marker by complex punctuation (`[MARKER]...; behaves`), and (2)
Delimiter Depth for a nested `[[MARKER]]` interacting with `MAX_EXACT_REFERENCE_DELIMITER_DEPTH`.
Suggested Next: read `generate_dialogue` (L695).

## What complete reading established

Every concrete claim is already grounded by committed source + regressions, several naming the exact
prior reports that proposed the identical tests against the **same source SHA** `902a0358…`:

- **c001 (Observed)** — verified exact. Preservation-only-if-reference-syntax is `remainder.push` gated
  on `reference_syntax.is_some()` at **L129-131**; occurrence type L29; orchestrator L114-144. The top
  scan is a single left-to-right pass with *nested* helper rescans, so "multi-pass" is loosely
  descriptive, not literally multiple full passes — no mechanism error.
- **c002 (Snag)** — non-manifesting. `split_whitespace` (L91) splits on Unicode White_Space (incl.
  U+00A0); the `trim_matches` closure (L92) is a non-alphanumeric **catch-all edge trim** (strips
  U+FEFF, U+00AD, punctuation runs); `find(|w| !w.is_empty())` skips fully-punctuation chunks — so the
  allowlisted verb is still isolated. Pinned by `…first_word_after_non_breaking_space` (tests.rs L3272,
  from report `1786936281`), `…internal_soft_hyphen` (L3325, from `1786999457`), and
  `…first_word_after_punctuation_boundary` (L3230). The one genuine limit — a format char **inside** the
  verb — fails **closed** (marker stripped), pinned L3348-3359. Concern preserved; hypothesis corrected,
  not domesticated.
- **c003 (Test 1, Relation Persistence)** — duplicate of Test 1 from `introspection_astrid_llm_1786814454`
  and `_1787135542`. Pinned by `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts`
  (L2932), `…preserves_relation_after_multiple_punctuation_runs` (L2884), `…grounds_first_word_after_punctuation_boundary`
  (L3230), `…quoted_context_precedes_following_relation_verb` (L2978). **Precision:** a bracketed
  `[MARKER]` is preserved via the **grouped** delimiter path (checked first, L50), not the relation
  path; a **bare** marker exercises the relation path. Both preserve byte-exact — the report's literal
  `[MARKER]...; behaves` example conflates the two paths.
- **c004 (Test 2, Delimiter Depth)** — duplicate of Test 2 from `introspection_astrid_llm_1786814454`.
  Pinned **exactly** by `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two`
  (L3025: `[[<end_of_turn>]]` → Grouped, depth 2), plus beyond-MAX bounding at L2266. Source: `before`/
  `after` each `take(MAX=4)` (no OOB); `delimiter_depth` is `count()` over a bounded `zip.take_while`,
  panic-free.
- **c005 (Suggested Next)** — read-only continuation pointer; confirmed `generate_dialogue` at L695. This
  is Astrid's own Tier-1 self-research (`NEXT: INTROSPECT astrid:llm 400`); no steward action taken.

## Disposition

No new source or test was written: the report re-derives concerns already fully grounded by committed
regressions naming the prior report IDs against the identical source SHA. Closed **addressed_duplicate**.
The felt/observational reading is preserved as primary evidence; the one contradiction (the snag's
"filter can't catch it" premise, and the bracketed-vs-bare path conflation in Test 1) is stated plainly,
not smoothed over. No live change; no restart/deploy required or attempted.
