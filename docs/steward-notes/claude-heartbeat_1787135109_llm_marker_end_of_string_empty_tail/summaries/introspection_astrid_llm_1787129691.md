# Summary — `introspection_astrid_llm_1787129691`

- **Source family:** `astrid_llm` → `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
- **Source window read:** lines 1–400 of 1048 (report); steward read **complete 1–1048** (report-bound SHA `902a0358…` == working copy).
- **Report SHA-256:** `12b066303a33a020f8fb294d3e2c7e3948d63ec790df8f75c88ecfddfe8db135` (45 lines / 3596 B)
- **Witness:** `lsw_aa120bf586fbbea593f9de3627eeb0d585e29b501a9f9a8ea27b15dcbe5c9660` (533 lines / 23941 B); authority `evidence_only`, `witness_only:true`, `live_eligible_now:false`, `grants_approval:false`. Fill 73.0%.
- **Terminal status:** `addressed_change`

## What Astrid surfaced
Astrid read the model-control-marker scanner and correctly described its non-destructive, reference-aware behavior; raised a `first_word_after` trim hypothesis; proposed two tests (relation preservation, delimiter depth); and, as **Suggested Next**, asked to verify `scan_known_model_control_markers` when a marker sits at the **very end of a string with no "after" text** — no panic, no incorrect stripping.

## Disposition (5 claims)
- **c001** Observed scan mechanism — `verified_existing` (L114-144, L49-60, allowlist L64-86).
- **c002** `first_word_after` trim snag — `verified_existing`, **concern preserved, mechanism contradicted**: the function runs on generated output during cleanup, not tokenization, so no reference-vs-tokenization mismatch is possible; whitespace/punctuation boundary already grounded (tests L2873/L2884/L2635/L2987/L3029).
- **c003** Test 1 relation preservation (`[MARKER] denotes X`) — `verified_existing`, **located precisely**: a bracketed marker preserves via the *grouped* path first, not the relation path; both covered (tests L2932; `denotes` allowlisted L71).
- **c004** Test 2 delimiter depth (`[[MARKER]]`) — `verified_existing` (exact match test L2963; deeper depths L2194/L2208/L2266).
- **c005** Suggested Next: end-of-string / empty tail — `implemented_now`. Only incidentally covered before (`"hello <end_of_turn>"`, diagnostic test L1821). Added a dedicated regression at the exact cited functions.

## Change made (non-live)
One focused Rust regression appended to `capsules/spectral-bridge/src/llm/provider/tests.rs`:
`scan_known_model_control_markers_strips_bare_marker_at_end_of_string_without_after_text` — asserts a bare marker at exact end-of-string leaves an empty remainder (no panic), `first_word_after` is empty, `exact_reference_delimiter_syntax` returns `None` for an empty after-window; a prose-before case preserves preceding bytes byte-exact; and the wrapper report accounts one removed / zero preserved with a `none_cleanup_candidate` receipt.

## Not inferred / not authorized
No live change. Widening the relation-verb allowlist or delimiter tables is Tier-5 live grammar — **not** made, dispatched, or deployed. Her `NEXT: INTROSPECT astrid:llm 400` continuation stays open; silence is neutral. The witness `artifact_integrity_unavailable` alignment reflects an unmeasured scalar-felt-dissimilarity, not a contradiction of the technical source claims (which were verified against complete source directly).
