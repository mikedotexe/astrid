# Summary — introspection_astrid_llm_1788024897

**Source:** `astrid:llm` → `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
(report-bound SHA `902a0358…`, current working-copy SHA `902a0358…` — **match**; complete
1–1048 read this round). **Report SHA** `3616fa6f…` (45 lines, 3722 bytes). **Witness**
`lsw_a1994694…` (533 lines, 23928 bytes, SHA `7c8c1f65…`), authority `evidence_only`,
`live_eligible_now=false`. Fill 71.1%.

## What Astrid surfaced

A fresh-pass re-read of the control-marker scanner window. She accurately describes
`scan_known_model_control_markers` (L114) rebuilding a clean remainder while tracking marker
metadata via a relational-verb allowlist (L64-86), then names two snags and proposes two tests:

1. **Unlisted synonyms** — `first_word_after` (L89-96) matches a hardcoded verb set; a synonym
   not in it (she names **"simulates"** and **"operates"**) may not be recognized as a reference,
   so the marker could be stripped.
2. **Delimiter depth** — `exact_reference_delimiter_syntax` (L199) could miscalculate scope on
   multi-byte UTF-8 or unusual nesting; she proposes a nested `[ [ marker ] ]` test.

She also suggests next reading `generate_dialogue` (L695) to see how the remainder integrates
into the output buffer.

## Disposition — all six claims `verified_existing`; report `addressed_duplicate`

- **c001 (observed mechanism):** accurate. Preserved a non-domesticating imprecision — visibility
  is also retained by quoted (L157-174) and grouped (L175-197) delimiters, not only relational
  verbs. Pinned by `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts`.
- **c002 / c004 (simulates/operates unlisted):** her observation is **factually true** (unlike the
  earlier `is` case): both verbs are genuinely absent from the L64-86 allowlist. But the
  unlisted-verb→`None`→strip path is a **deliberate bounded allowlist**, already proven verb-
  agnostically by `distinguishes_allowlisted_is_from_unlisted_acts`, `…_signals_from_unlisted_signifies`,
  and the `does_not_expand_relation_allowlist_to_{implies,contains,creates,triggers,…}` guards.
  Prior packets `1787707869` (operates) and `1786932936` (operates synonym) already declined a
  redundant unlisted-verb test; consistency keeps that stance. Concern preserved as a true reading
  of an intentional design fact. **Widening the allowlist would be a Tier-5 grammar change — not
  authorized here.** No new test authored (would duplicate the passing unlisted-verb class).
- **c003 / c005 (delimiter depth):** her exact proposed test already exists —
  `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (`[[<end_of_turn>]]`
  → grouped, depth 2). Multi-byte UTF-8 covered by the fullwidth/CJK stack tests; the
  "premature termination" concern covered by the beyond-max square-bracket bounding test. The
  function iterates by `.chars()` (byte-safe) and bounds depth to `MAX=4`.
- **c006 (generate_dialogue integration):** her own read-only next step (agency continues).
  Grounded note: the sanitizer feeds only the **validation gates** (`is_valid_dialogue_output`,
  `has_one_nonempty_final_next_action`); `generate_dialogue` returns the **original** model text
  on pass, so the scan is non-destructive to the emitted buffer.

## Verification

84 focused tests passed, 0 failed (`marker`/`delimiter`/`first_word`/`relation` filters) at
source SHA `902a0358…`. No source change; no live change. This is a review/verification round —
restart and deployment were not required or attempted.
