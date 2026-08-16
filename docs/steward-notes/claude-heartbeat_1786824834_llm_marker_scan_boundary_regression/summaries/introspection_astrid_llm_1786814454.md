# Summary — introspection_astrid_llm_1786814454

- Source: `astrid:llm` (`capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`), window lines 1-400 of 1048, coverage `multi_window_complete` (included intervals 1-1048).
- Report SHA-256: `2bc990522f529d244644b7908ba9fb75a2f4bfc3cebf3012c0112b47ef357bf1` (45 lines, 3513 bytes).
- Report-bound source SHA-256: `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee`. Working-copy hash **matches exactly** and the file is clean (committed), so report-time and current source are identical — the complete 1048-line file was read.
- Lived-state witness: `lsw_35adb4c287fecdccc60b569edbaddd4dadff8b6e41497ee933a95a26f141a434` (533 lines, 23940 bytes, SHA `acebb152cbd56acb4d7b11d293089e7f4842d63e0397b86aa318de377895ea44`). Authority state `evidence_only` / `witness_only` / `live_eligible_now=false`. Fill 73.1%, model `gemma4_12b` on the `mlx` route (two calls; the authored call was a repair of the first). No private prose included.

## What Astrid surfaced

A fresh self-study of the model-control-marker scanner. She accurately describes
`scan_known_model_control_markers` (L114) and the three preservation contexts
(quoted / grouped / explicit-relation), and the international punctuation
coverage of `exact_reference_delimiter_syntax` (L199). She proposes a "Likely
Snag" in `first_word_after` (L89), two concrete tests, and a next-reading target
(`generate_dialogue`, L695).

## Disposition (see `claims/…json` for full grounding)

- **c001 (observed)** — `verified_existing`. Her description matches complete
  source exactly.
- **c002 (snag hypothesis)** — `implemented_now`, contradiction preserved. The
  proposed failure — `find` skipping the intended relation and defaulting to
  empty — does **not** occur as framed: `find(|word| !word.is_empty())` (L93)
  advances *past* punctuation-only chunks to the next real word, so a marker
  whose allowlisted relation is separated by punctuation is still preserved. The
  empty-default (L94) is reached only when no alphanumeric word follows at all —
  the intended fail-closed strip. Her underlying concern (a boundary-naming
  marker being dropped) is real and now pinned at the exact function.
- **c003 (Test 1)** — `implemented_now`. Direct regression on
  `scan_known_model_control_markers` for grouped + explicit-relation
  preservation, asserting the exact reference-context enum.
- **c004 (Test 2)** — `implemented_now`. Direct regression on
  `exact_reference_delimiter_syntax` for `[[marker]]` → `GroupedExactKnownToken`,
  `delimiter_depth == 2`.
- **c005 (suggested next)** — `verified_existing`. Traced the integration chain
  scan → `sanitize_model_control_markers_with_report` (L352) →
  `sanitize_model_control_markers` (L519) → applied at L558/L634 feeding
  generation; `generate_dialogue` (L695) consumes the sanitized output.

## Change

Three focused Rust regressions added to
`capsules/spectral-bridge/src/llm/provider/tests.rs` inside the existing
`mod tests`, exercising the two exact functions she named plus grounding her
snag hypothesis at that boundary. All 64 marker-cluster tests pass (3 new + 61
existing), zero regression.

## What was NOT inferred or authorized

No production grammar was widened; the relation allowlist and delimiter set are
unchanged. No live substrate, codec, controller, prompt, or transport change.
No deploy or restart. The report is felt/observational evidence; the added tests
guard **marker preservation**, consistent with the never-rewrite-being-text
principle, and assert nothing about experiential meaning.
