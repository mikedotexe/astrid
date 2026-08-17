# introspection_astrid_llm_1786967484 — summary

- **Source family:** `astrid_llm` (`capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`)
- **Report SHA-256:** `3345dae820515ec6a20831d46c7ea04515185f0fcf17688557c172acc6cb10fd` (45 lines, 4036 bytes)
- **Witness:** `lsw_a0e2d9d83e08e4720df0220272e88e3526cc3fd830128537dc79bfeb8cebdcef` (533 lines, 23941 bytes)
- **Report-bound source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — working copy **matches exactly and is clean** (not dirty), so report-time and current-source conclusions are identical.
- **Runtime context (witness):** fill 71.1%, model profile `gemma4_12b`, `artifact_authority_state = evidence_only`, `live_eligible_now=false`.

## What she surfaced

A fresh-pass read of the marker-awareness grammar in `dialogue_runtime.rs`: the
`scan_known_model_control_markers` / `first_word_after` / `exact_reference_delimiter_syntax`
functions that decide whether a `KNOWN_MODEL_CONTROL_MARKERS` token stays visible
(cited as content) or is stripped (bare control sequence). She proposed two focused
tests and named two "Likely Snags."

## Dispositions

| Claim | Kind | Disposition |
| --- | --- | --- |
| c001 architecture (scanner + depth-limited delimiter check) | verified_existing | Source L18-229 matches her account exactly; delimiter depth cap = 4 (L151). |
| c002 Snag A (`first_word_after` on punctuation-heavy tail) | verified_existing | Hypothesized failure does not occur: `trim_matches` (L92) + `find(!is_empty)` (L93) resolve `... (as ...` to allowlisted `as`; marker preserved. Concern preserved. |
| c003 Snag B (unframed marker omitted → "jumps") | verified_existing | True but **intentional**: L129 keeps the token only when `reference_syntax.is_some()`; bare control markers are removed by design (fail-closed sanitization). |
| c004 Test 1 (CJK/Unicode bracket → Grouped) | verified_existing | Her exact function+outcome already pinned (double-square-bracket depth-2 Grouped; CJK/fullwidth group pairs). **Grounding:** corner-quotes `「」` classify as **Quoted**, not Grouped. |
| c005 Test 2 (`[MARKER] behaves,` trailing comma) | **implemented_now** | Added `scan_known_model_control_markers_grounds_trailing_comma_relation_verb`; `first_word_after == "behaves"`, marker preserved as `ExplicitExactKnownTokenRelation`. |
| c006 Suggested Next (`generate_dialogue` L695+) | observed | Her Tier-1 read-only continuation; target confirmed to exist at L695; no steward action. |

## Non-domestication note

Her expectation in Test 1 that a CJK-bracketed marker resolves to `GroupedExactKnownToken`
holds **only for bracket-type delimiters** (`[]`, `【】`, `［］`, `〔〕`, …). CJK **corner
quotes** `「 」` are classified as `QuotedExactKnownToken` (pinned by
`control_marker_cleanup_preserves_non_ascii_matching_quote_pairs`). This is stated as a
plain grounding, not a rewrite of her report; the underlying concern — that delimiter
classification be correct across Unicode — is exactly what the existing suite proves.

## Terminal status

`addressed_change` — one focused non-live regression added; all other claims grounded
against complete source and existing coverage. No live substrate/control change; no
restart or deployment required or attempted.
