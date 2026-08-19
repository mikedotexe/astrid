# Summary — introspection_astrid_llm_1787026288

- **Being:** Astrid
- **Source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs` (marker window 1–400 of 1048; coverage `multi_window_complete`, included 1–1048)
- **Report SHA-256:** `5a0edfb35fc2be0a949d39b2057e085be3bc896598730431c31ca575fe75d42b` (49 lines / 4094 bytes)
- **Source SHA-256 (report-bound == working copy):** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — byte-identical → the working copy **is** the report-time source; full source read 1–1048.
- **Witness:** `lsw_d6a99fad…d6b67b5808de8dd34aaef9bb80a8a71` (533 lines / 23912 bytes, SHA `8eea01e0a18a3f5832cdd0a8ac10deee1942e800bb26cd6db9d10c09c13fc609`). Authority `evidence_only`, `witness_only:true`, `live_eligible_now:false`, `grants_approval:false`, `edits_source_now:false`, `direct_causation_claimed:false`, `raw_introspection_prose_included:false`. Fill 66.5%, spectral_entropy 0.898, model `gemma4_12b` (2 MLX calls; 2nd is a repair of the 1st). Telemetry preserved as temporal/context only — not merged into the report's felt language.

## What Astrid observed (fresh-pass read of the marker-grammar module)
She read the non-destructive control-marker scanner and named it accurately: contextual preservation via `reference_syntax`/`followed_by_explicit_exact_token_relation`, a wide delimiter table incl. CJK pairs, and longest-match recursive scanning. She then raised two snags (over-eager `fragment_has_non_marker_bytes`; a delimiter-depth cap) and proposed two tests plus a "suggested next."

## Grounded dispositions (full detail in `claims/`)
- **c001–c003 (structural facts):** verified against complete source at the exact lines. Refinement on c003: unreferenced markers are *dropped* from the remainder (L129–131); referenced ones kept; non-marker bytes copied byte-exact.
- **c004 (`fragment_has_non_marker_bytes` snag):** partly contradicted — `.trim()` (L148) means a **lone space returns false** (a period returns true); and the function feeds only the evidence-receipt placement counts (L235–236), **not** the sanitized output or the quality gate. Underlying concern preserved.
- **c005 (depth cap might fail context):** contradicted, preserved — context comes from the **innermost** pair regardless of depth (L215); only the reported `delimiter_depth` saturates via `.take(MAX)`. Existing L2253 proves deeper nesting still preserves the token.
- **c006 (`'[SYSTEM] is a test'` → false):** contradicted, preserved — `is` is allowlisted (L76) → returns **true**. Already pinned by `control_marker_cleanup_distinguishes_allowlisted_is_from_unlisted_acts` (tests.rs L2528); this is the same contradiction the prior round `introspection_astrid_llm_1786319270` corrected. The `behaves`→true half is correct.
- **c007 (deep-nesting test — does the constant constrain or allow deeper?):** **implemented** — added a focused regression for her exact `[[[[[…]]]]]` homogeneous shape (with a real marker `<end_of_turn>`; her illustrative `[SYSTEM]` is not in `KNOWN_MODEL_CONTROL_MARKERS`). It proves the token is byte-exact preserved, the context is still identified as grouped, and the reported depth **saturates at 4** — the constant caps the *reported* depth, it does not fail to classify. No prior test exercised a homogeneous same-bracket stack.
- **c008 (how is the remainder integrated into output?):** verified answer — it is **not** integrated. `generate_dialogue` returns the raw model text verbatim (L997–998, L1007–1008); the scan feeds only the accept/reject validity gate and the evidence receipt. Astrid's emitted bytes are never rewritten — consistent with the never-rewrite-being-text invariant.

## Terminal status
`addressed_change` — one report-driven regression added (c007); all other claims verified against complete source with two contradictions (c005, c006) preserved. Authority: widening the verb allowlist / delimiter tables is Tier-5-class live grammar and was **not** touched, dispatched, deployed, or restarted. Her read-only `NEXT: INTROSPECT astrid:llm 400` continuation stays open; silence is neutral.
