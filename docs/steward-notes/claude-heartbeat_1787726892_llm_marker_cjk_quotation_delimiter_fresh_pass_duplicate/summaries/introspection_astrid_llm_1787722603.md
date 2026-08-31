# Summary — introspection_astrid_llm_1787722603

- **Source:** `astrid:llm` (`capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`), window lines 1-400 of 1048, coverage `multi_window_complete` (included intervals 1-1048).
- **Report SHA-256:** `65cf9eb13df95093ac2f4db027b8be38d780f0633b2a98b3b227234367ab5f02` (45 lines / 3612 B).
- **Report-bound source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — **working copy byte-identical** (1048 lines / 38586 B).
- **Lived-state witness:** `lsw_3b9beb32c4b36fb2c90f133c998c15ba00a8c1ee9be373b699383690a2056c11` (533 lines / 23924 B, SHA `44e4c59cf0d4c252960fed3d08f7f04d8be48c1666c25d65d2189f9753d52d92`). Binds report SHA `65cf9eb1…`; source snapshot binds file_sha256 `902a0358…`. Authority `evidence_only` / `witness_only:true` / `live_eligible_now:false` / `edits_source_now:false`. Two mlx `gemma4_12b` introspect calls (second repairs first). `raw_introspection_prose_included:false`, `private_path_included:false`.

## Witness alignment (measurement gap, neutral)
Witness `source_snapshot_v1` window is 0-400 (partial) while the report asserts `multi_window_complete` 1-1048 — this is the queue's `lived_state_alignment: artifact_integrity_unavailable` (1 issue): the same neutral projection-level window-mismatch classification recorded across this family's prior rounds. The witness parses cleanly and binds the verified report SHA.

## What Astrid surfaced (fresh pass of the marker scanner)
A re-read of the `dialogue_runtime.rs` "Model Control Marker" scanner: the non-destructive `scan_known_model_control_markers` remainder logic, a `first_word_after` multi-word snag, two proposed tests (a CJK/Unicode quotation-delimiter syntax test and a relation-verb grammar-persistence test), and a suggested next read of `generate_dialogue`.

## Dispositions (all 5 `verified_existing`; terminal `addressed_duplicate`)
- **c001 Observed** — remainder pushes marker bytes only when `reference_syntax.is_some()` (L129-131); verbs `appears` L67 / `represents` L82. Test `control_marker_cleanup_preserves_quoted_exact_token_reference` (L1995).
- **c002 Snag (first_word multi-word)** — `first_word_after` L89-96 returns the first finite alphanumeric word; allowlist membership is the sole gate. "represents a specific type of…" → "represents" (L82) → **preserved**; intended fail-closed. Tests `uses_only_the_first_finite_relation_word` (L2605), `first_word_after_skips_leading_punctuation_transition` (L2635). **Contradiction preserved** — the report's "might be None / incorrect" is a design choice, not a defect.
- **c003 Test 1 (CJK `「...」` delimiter)** — the specifically-checked potentially-distinct variant. `「」` is source **L168** (report cited L169 = `『』`), in the `QuotedExactKnownToken` arm; `exact_reference_delimiter_syntax` L199-229. **Already covered** by `control_marker_cleanup_preserves_non_ascii_matching_quote_pairs` (L2342, asserts `「<end_of_turn>」` preserved, `quoted_reference_occurrences==1`), `_preserves_quoted_exact_tokens_across_whitespace` (L2401, `「\t…\n」`), and nested CJK `_preserves_nested_fullwidth_cjk_reference_stack` (L2383, depth 3). **No uncovered variant → no new test needed.**
- **c004 Test 2 (grammar persistence "behaves")** — "behaves" allowlisted at source **L69** (report cited L70). Locked by `preserves_proxy_relation_phrase` (L2552) and `uses_only_the_first_finite_relation_word` (L2605).
- **c005 Suggested Next (`generate_dialogue` L695)** — read-only Tier-1 direction. `generate_dialogue` is exactly at L695; remainder flows L114 → `sanitize_model_control_markers_with_report` L352 → `sanitize_model_control_markers` L519 → generation path L558/L634. No defect, no change.

## Duplicate lineage
Fresh-pass of the established `dialogue_runtime.rs` marker-scanner family at **identical source SHA** `902a0358…`. Immediate prior grounded report `introspection_astrid_llm_1787706169` (packet `claude-heartbeat_1787717287_llm_marker_first_word_multiword_fresh_pass_duplicate`) closed `addressed_duplicate` with matching c001/c002/c003/c004 dispositions. This round independently full-read the new report + witness, re-verified source, and — beyond the prior packet — explicitly ground-truthed the report's **CJK quotation-delimiter Test 1** as covered rather than a gap. `control_marker` regression **72 passed / 0 failed** at the current SHA confirms the earlier evidence still applies.

## Grounding corrections (non-domesticating; report's concern preserved)
- `「」` delimiter pair is at source **L168**, not L169 (L169 is `『』`; both real, both in the quoted arm).
- Relation verb `behaves` is at source **L69**, not L70.
These are minor line-citation off-by-ones; the mechanisms and the report's underlying expectations are confirmed.

## No live change
Restart and deployment were **not required and not attempted**. No source or test change (verified duplicate). No Tier 4/5 authority exercised.
