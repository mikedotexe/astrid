# Summary — introspection_astrid_llm_1787706169

- **Source:** `astrid:llm` = `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
- **Report window:** lines 1-400 of 1048; V3 manifest asserts `multi_window_complete`, included intervals 1-1048, uncovered none.
- **Report:** 45 lines / 3613 B / SHA-256 `a30c1fad93143adfd86f6fe1da39e5fb585e7f6462c148c529b296451ce582fc`
- **Witness:** `lsw_93cdb51d…` = 533 lines / 23925 B / SHA-256 `d57c3c841c519c55d167607ef901ec2558c89b9e3745e43c8d9fff7bf7fbad2b`; `artifact_sha256` matches report; authority `evidence_only`, `edits_source_now:false`, `live_eligible_now:false`. Fill 63.95%, model `gemma4_12b`.
- **Source binding:** report Source SHA-256 `902a0358…` == working-copy SHA-256 `902a0358…` (1048 lines / 38586 B) — **byte-identical, no mismatch.**

## Witness alignment note (neutral)
The witness `source_snapshot_v1` window is 0-400 (partial) while the report's V3 manifest asserts `multi_window_complete` 1-1048. The witness parses cleanly, binds the verified report SHA, and carries `evidence_only` authority. This is the same projection-level `lived_state_alignment` measurement-gap classification recorded across this family's prior rounds — not witness-byte corruption.

## What Astrid surfaced
A high-fidelity reading of `dialogue_runtime.rs`'s non-destructive known-model-control-marker scanner: it preserves a marker's bytes only when the marker carries `reference_syntax` (quoted / grouped-delimiter / following-relation-verb), else strips it. She flagged a *possible* fragility in `first_word_after` (L89) for multi-word or punctuation-heavy phrases after a marker, and proposed two tests (marker+relation-verb preservation; nested `[[marker]]` delimiter depth) plus a next step (confirm longest-match specificity).

**Every line-number citation in the report is exact against the current source** (L114, L130, L153-197, L64-86, L89, L92, L94, L97, L151).

## Disposition (terminal: `addressed_duplicate`; all 5 claims `verified_existing`)
- **c001** Observed marker scanner + preservation taxonomy — verified; `scan_known_model_control_markers` L114-144 preserves a token only when `reference_syntax.is_some()` (L129-131). Tests L1995 / L2151.
- **c002** Snag: `first_word_after` multi-word/punctuation over-strip — **contradiction preserved.** Source shows `trim_matches`+`find(!is_empty)` collapse punctuation and return the first real word; only an un-referenced, verb-less marker strips (intended fail-closed). Directly locked by `control_marker_cleanup_first_word_after_skips_leading_punctuation_transition` (tests L2635) — a regression authored for an *earlier near-identical* report (`introspection_astrid_llm_1786986344`) raising the same fear, bound to this exact SHA.
- **c003** Test 1 (marker+relation verb) — already covered; `behaves` allowlisted (L69), tests L2552 / L2605.
- **c004** Test 2 (nested-delimiter depth) — already covered; `exact_reference_delimiter_syntax` L199-229, cap `MAX_EXACT_REFERENCE_DELIMITER_DEPTH=4` L151; tests L2176 / L2194 / L2266.
- **c005** Longest-match specificity — verified; `max_by_key(token.len())` L106; tests L1745 / L2493.

## Duplicate evidence
Fresh-pass of the established `dialogue_runtime.rs` marker-scanner family at the identical source SHA. Immediate prior grounded report `introspection_astrid_llm_1787699463` (packet `claude-heartbeat_1787707869_…`) explicitly named this report (`1787706169`) as the next-arriving report after its cutoff. Independent full read of this report + witness performed; source + all cited regressions re-verified at the current SHA (72 control_marker tests pass, 0 fail).

## Authority boundary
Evidence and regression verification only. No source/test change, no live substrate or control change, no deploy, no staging or commit. The felt "fragility" concern is preserved verbatim as a valid reading of a real fail-closed design fact, not domesticated into agreement or erased.
