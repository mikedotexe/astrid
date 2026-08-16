# Summary — introspection_astrid_llm_1786885842

Astrid re-read `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
(source SHA `902a0358…`, coverage `multi_window_complete` 1-1048) and reported on
the KNOWN_MODEL_CONTROL_MARKERS preservation grammar. Fill 71.0%. This is a
fresh-pass re-read of the same source window addressed the round immediately
before (`introspection_astrid_llm_1786858484`, closed `addressed_duplicate`).

## What she said
- **Observed (c001):** the module identifies control markers and preserves them
  based on grammatical context — Quoted / Grouped / Explicit reference contexts.
- **Snag (c002):** `first_word_after` (L89-96) uses a hardcoded verb list
  (L66-85). If a model uses an unlisted synonym (she names "symbolizes"),
  `followed_by_explicit_exact_token_relation` returns false and the marker "might
  be stripped when it should have been preserved." She correctly noted many
  synonyms (denotes, indicates, manifests, represents, corresponds) ARE present.
- **Test 1 (c003):** feed " [MARKER] symbolizes X" and check preserve-vs-strip.
- **Test 2 (c004):** verify `exact_reference_delimiter_syntax` (L199-229) handles
  nested `[[MARKER]]` and that `delimiter_depth` ≤ `MAX_…DEPTH` (L151=4).
- **Suggested Next (c005):** examine `generate_dialogue` (L695+) for how the scan
  remainder reaches the output buffer.

## What complete source + tests establish
Every concrete claim is `verified_existing` against the current source and the
standing 65-test marker suite (all green at this SHA):

- **c001** verified — scan (L114-144) preserves a token only when
  `reference_syntax.is_some()`; taxonomy enum L42-46.
- **c002** verified, **contradiction preserved**. A bare, undelimited marker + an
  unlisted verb genuinely IS stripped — but that is the *intended fail-closed*
  design, not the "should have been preserved" defect she frames. Preservation
  requires a proven reference (an allowlisted relation OR a delimiter); an
  unlisted verb is not proof. Her real, un-domesticated concern — the relation
  allowlist is finite, so a legitimately-referential sentence using an unlisted
  verb loses the marker — is preserved as a Tier-5-class live-grammar question,
  not silently agreed-with or waved off.
- **c003** verified — the exact boundary is locked by
  `…distinguishes_allowlisted_is_from_unlisted_acts` (L2528) plus the
  `does_not_expand_relation_allowlist_to_{implies,contains,creates,triggers}`
  family (L2595-2655). Her "symbolizes" is the same class as the already-tested
  "acts". No redundant test added.
- **c004** verified — `…double_square_bracket_depth_two` (L2876, her exact case),
  `…reports_exact_four_level_delimiter_depth` (L2194, =MAX), and
  `…bounds_deeper_delimiter_receipt_without_dropping_token` (L2253, 5 nested →
  bounded at 4, kept) directly prove "does not exceed MAX".
- **c005** verified — the scan remainder feeds only the validity predicates
  (L558/L634 local `stripped` copies) and `sanitize_…_with_report` (L352);
  `generate_dialogue` (L695) returns raw model text. Her read-only continuation
  stays open.

## Disposition
Terminal status **`addressed_duplicate`** of `introspection_astrid_llm_1786858484`
(same window, same SHA, all-`verified_existing`), with mechanism-lineage anchors
`introspection_astrid_llm_1786871574` / `…_1786848204` / `…_1786319270` (the
origin of the allowlisted-`is`-vs-unlisted-`acts` regression). Current
verification: focused marker tests 65 passed / 0 failed at source SHA `902a0358`
unchanged; independent full read of report + witness completed.

No live change. No new test (the boundaries are already locked). Widening the
relation allowlist or delimiter tables or raising MAX depth remains Tier-5 and
was not made.
