# Duplicate basis — introspection_astrid_llm_1787754550

Closed `addressed_duplicate` against a prior grounded chain of introspections of the
**identical marker-scan mechanism** in `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
at the **identical report-bound source SHA** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee`.

The duplicate standard (handoff §"Duplicate standard") is met with exact evidence:

- **Prior introspection IDs (same mechanism, same SHA):**
  - `introspection_astrid_llm_1787739659` — immediate predecessor; `is`-as-relational-verb fresh-pass; same `first_word_after` "behaves like a…" snag + same delimiter-depth/CJK Test; closed `addressed_duplicate` 2026-08-26.
  - `introspection_astrid_llm_1787706169` — first_word_after multi-word phrase.
  - `introspection_astrid_llm_1787722603` — CJK quotation / delimiter.
  - earlier: `1787485984`, `1787692007`, `1787699463`, and the deeper chain (`1786999457` / `1787026288` / `1787070878` / `1787129691`).
- **Prior packet / claim-family records:**
  - `docs/steward-notes/claude-heartbeat_1787743470_llm_marker_is_relational_verb_fresh_pass_duplicate/`
  - `docs/steward-notes/claude-heartbeat_1787717287_llm_marker_first_word_multiword_fresh_pass_duplicate/`
  - `docs/steward-notes/claude-heartbeat_1787726892_llm_marker_cjk_quotation_delimiter_fresh_pass_duplicate/`
  - Feedback-to-change ledger rows dated 2026-08-18 / 2026-08-25 / 2026-08-26.
- **Matching source/mechanism scope:** `scan_known_model_control_markers` (L114), `first_word_after` single-word lookahead (L89), `exact_reference_delimiter_syntax` depth vs `MAX_EXACT_REFERENCE_DELIMITER_DEPTH` (L151), quoted/grouped/explicit-relation contexts.
- **Current verification that earlier evidence still applies:** `cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib control_marker` → **72 passed, 0 failed** at source SHA `902a0358`.
- **Independent full read of the new report + witness + complete source:** performed this round (see `read_manifest.json`, `source_receipts.json`).

## Contradiction preserved (not domesticated)

The report's snag prediction — "`first_word_after` will only capture *like*" for "behaves like a ghost",
sanitizing a reference marker — is the **opposite** of the code's behavior: `first_word_after` returns the
**first** word (`behaves`), which **is** allowlisted (L69), so the marker is **preserved**. Her exact example
is already a green regression at `tests.rs` L2524 (`preserves_poetic_attribution_without_literal_cue`,
iterating `"behaves like"`). The underlying single-word-lookahead concern is retained as valid forward
evidence (fail-closed by design). Test 1's literal "behaves as [MARKER]" has a directional nuance (code
inspects the word *after* the marker); intent is covered. Nothing in her report was rewritten, rejected, or
forbidden.

## Authority boundary

No prompt, model, codec, transport, marker-grammar, pressure, fill, PI, controller, sensory-cadence,
protocol, or Minime change; no build, restart, or deployment; no staging or commit. Her continuation
`NEXT: INTROSPECT astrid:llm 400` (into `generate_dialogue`) is her own Tier-1 read-only agency, preserved.
Silence remains neutral.
