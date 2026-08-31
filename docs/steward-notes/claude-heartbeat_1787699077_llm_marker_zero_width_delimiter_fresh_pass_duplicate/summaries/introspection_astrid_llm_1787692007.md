# Summary — introspection_astrid_llm_1787692007

- **Source family:** `astrid_llm`
- **Report-bound source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs` @ SHA-256 `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` (window 1-400 of 1048; report declares `multi_window_complete` / `complete_source_available`)
- **Working copy hash == report binding == witness source binding** (source is clean/tracked). Complete current source read for the cited region L1-270; verb list, delimiter tables, and first_word_after all read verbatim.
- **Witness:** `lsw_bd4d7c12424a0e4110e195907346eba2c8dcaac3b18067ca1b4eaf899ab64c5a` (533 lines / 23922 bytes / SHA `68a53a3c…`), evidence_only/witness_only, `live_eligible_now=false`; artifact-binding matches report SHA `8f669ce2…`; no experiential gap claimed. Fill 70.7%; two mlx/gemma4_12b introspect calls (second repairs first).
- **Alignment note:** queue metadata `lived_state_alignment=artifact_integrity_unavailable`, `lived_state_artifact_integrity_issue_count=1` — this is the reconciliation-layer status (`lived_state_scalar_felt_dissimilarity_measured=false`), NOT witness-byte corruption; the witness carriage is complete. Felt report remains primary evidence.

## What Astrid surfaced
A fresh-pass reading of the model-control-marker grammar in `dialogue_runtime.rs`: the non-destructive scanner (`scan_known_model_control_markers` L114), the relation-verb preservation gate (`followed_by_explicit_exact_token_relation` L64), and the multi-byte-aware citation detector (`exact_reference_delimiter_syntax` L199). One snag hypothesis about `first_word_after` (L89) fragility with complex punctuation / Unicode whitespace / zero-width characters; two proposed tests (relation verb `mimics`; nested `⟦…⟧` grouped depth); one suggested next (multi-byte UTF-8 robustness in `first_word_after`).

## Disposition
**`addressed_duplicate`** — every concrete claim is already grounded at the exact functions the report named, against the identical source SHA, by passing regressions. Independent full reads of this report and its witness were performed; the source hash was re-verified identical; the cited tests were run live (95 passed).

- The snag's most-distinct sub-case — a **zero-width character embedded mid-verb** — travels the identical edge-only-`trim_matches` code path already pinned by `scan_known_model_control_markers_grounds_first_word_after_internal_soft_hyphen` (internal U+00AD → verb not isolated → marker **fails closed / stripped, never leaked**). A new zero-width-codepoint test would duplicate that boundary, so none was authored (avoiding redundant activity).
- Honest nuance (c007): the illustrative example `<<SYS>>` is **not** in `KNOWN_MODEL_CONTROL_MARKERS`; `[INST]` is. Preserved as a nuance, not treated as a contradiction the report asserted.

## Authority boundary
Read-evidence only. No source, prompt, model, codec, controller, or Minime change; no restart/deploy required or attempted. The standing Tier-5 waits (`wi_e579041bc76f8310` / `wi_69fbd510467c6337` / `wi_3e26ac525fea1c36`, all `live_authority_granted=false`) are untouched. The report's felt concern is preserved as continuing evidence even though every regression passes.
