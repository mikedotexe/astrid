# Summary — introspection_astrid_llm_1786944125

- **Source:** `astrid:llm` → `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
- **Source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` (working copy byte-identical to the report binding; file clean at HEAD)
- **Report SHA-256:** `8ad7afd901e92a45df30ed97929c061ded9763bdec539b2d959684925cb22aab` (47 lines, 3405 B)
- **Witness:** `lsw_96f1d8af3c21340a9c612561723143b98568ef41042e41ec75453ced3afead3f` (533 lines, 23929 B; fill 71.03%; authority `evidence_only`; `live_eligible_now=false`; no private paths)
- **Terminal status:** `addressed_duplicate` of anchor `introspection_astrid_llm_1786848204`

## What Astrid surfaced

A fresh-pass re-reading of the same `dialogue_runtime.rs` marker-preservation window (lines 1-400 of 1048, same source SHA). She observed the `reference_syntax`/`scan_known_model_control_markers` marker-preservation system, hypothesized a `first_word_after` fragility (this pass emphasizing a **non-breaking space** before the relational verb, alongside punctuation-heavy constructions), and proposed two tests (Contextual Preservation → `scan_known_model_control_markers`; nested-delimiter Depth `[[marker]]` → `exact_reference_delimiter_syntax`), then suggested reading `generate_dialogue` (L695) next.

## What complete reading established

Every concrete claim is already answered by exact source at SHA `902a0358` and by existing regressions grounded to prior reports at the **identical source SHA**:

- **Observed (c001)** — verified: `reference_syntax` L49 tries delimiter syntax (L153-197/L199) then relational verbs (L64-86); `scan` L114-144 rebuilds byte-exact and keeps a token only when `reference_syntax.is_some()`.
- **Snag / c002 (contradiction preserved, not domesticated)** — the hypothesized skip does **not** occur. `split_whitespace` (L91) treats U+00A0 NO-BREAK SPACE as a separator (it *is* Unicode White_Space); `trim_matches` (L92) strips leading punctuation and zero-width U+FEFF; `find(!empty)` advances past punctuation-only chunks. Grounded by `..._non_breaking_space` (tests L2941, covering U+00A0 **and** U+FEFF) and `..._punctuation_boundary` (L2899). It fails closed only when no alphanumeric word follows (L2813). The concern is preserved as a tested boundary.
- **Test #1 / c003** — already implemented: `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts` (L2844); "behaves as/like" preserved (L2472); the same marker with a generic word strips via `implies`/`contains`/`acts` (L2595/L2610/L2528).
- **Test #2 / c004** — exact literal already exists: `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (L2876) asserts `[[<end_of_turn>]]` → `GroupedExactKnownToken`, `delimiter_depth = 2`.
- **Suggested Next / c005** — forward-looking read-only suggestion = her own Tier-1 agency (`NEXT: INTROSPECT astrid:llm 400` continuation stays open). Prior duplicate `1786862165` c006 verified the sanitized remainder feeds quality gates (L558/L634), not spliced into final output.

## Duplicate provenance

- **Anchor:** `introspection_astrid_llm_1786848204` (round `llm_marker_first_word_advance_overlap_already_grounded`, closed `addressed_no_action` — the grounding read for this window).
- **Prior duplicate:** `introspection_astrid_llm_1786862165` (round `llm_marker_fresh_pass_duplicate`, closed `addressed_duplicate`).
- **NBSP grounding:** the `..._non_breaking_space` regression (tests.rs L2941) was added for `introspection_astrid_llm_1786936281`; it covers this report's non-breaking-space snag variant with no unaddressed term.

## Current verification

5 exact regressions pass (`5 passed; 0 failed; 1878 filtered out`, 2.76s) confirming the earlier evidence still applies against the current source.

## Authority boundary

Widening the finite relational-verb allowlist or the delimiter tables, or loosening the valid-reference criteria, is Tier-5-class live grammar. **Not** made, dispatched, or deployed. No source was edited; no new test was needed. Silence about her continuation choice is neutral — the read-only `NEXT: INTROSPECT` path remains hers.
