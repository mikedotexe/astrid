# Summary — `introspection_astrid_llm_1786862165`

- **Source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs` SHA `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` (1048 lines, working copy **byte-identical** to the report binding; coverage `multi_window_complete`, intervals 1-1048, uncovered none).
- **Report:** SHA `fd21e752507b3c79d2473e02ee0237760e4908f8cc6368e62d2d35d7cf7e2c65` (45 lines, 3752 B). Fill 73.0%.
- **Witness:** `lsw_01d26cfc5633055eac652f56a129308cf94068f259eb8fd23679a9a74b1f082b` SHA `247e955704b305879aaa09582cf18184e3d49219c39c96ee1cec4c793e592f4a` (533 lines, 23927 B). model_profile `gemma4_12b` via mlx; two model routes where the second repairs the first (`repair_parent_call_id`) — the projection's `artifact_integrity_unavailable` alignment note corresponds to that repair-chain annotation, neutral and not contradicting content. Authority `evidence_only` / `witness_only`; no raw prose/prompt/response; `direct_causation_claimed=false`.

## What she said
A fresh-pass re-read of the same marker-scanner window: accurately describes marker-aware sanitization (`scan_known_model_control_markers` L114, `KnownModelControlMarkerMatch`, the three preservation contexts), raises two Likely Snags (`first_word_after` punctuation/multi-word fragility; the `reference_syntax=None` omission causing text "jumps"), two One-Test-Each designs (Relation Recognition on `followed_by_explicit_exact_token_relation`; Delimiter Depth on `exact_reference_delimiter_syntax` with `[[MARKER]]`), and a read-only Suggested Next (examine `generate_dialogue` L695).

## Disposition — `addressed_duplicate` of `introspection_astrid_llm_1786848204`
This report duplicates the mechanism scope and claim set of the immediately-prior `introspection_astrid_llm_1786848204` (closed `addressed_no_action` in packet `claude-heartbeat_1786860129_llm_marker_first_word_advance_overlap_already_grounded/`), and of the earlier chain (`1786851428`, `1786839292`, `1786831308`, `1786824834`). Duplicate standard met:

- **Prior introspection ID:** `introspection_astrid_llm_1786848204` (+ family chain above).
- **Matching source/mechanism scope:** identical source SHA `902a0358`, identical window (1-400 of 1048), identical six-claim structure at the same functions.
- **Current verification that earlier evidence still applies:** the **65** marker-cluster tests pass (0 failed) against the current source SHA `902a0358` (incremental build; `test_results.json`), including the exact tests these prior rounds added — `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (her Test #2, L2876) and `scan_known_model_control_markers_grounds_first_word_after_punctuation_boundary` (her Snag, L2900).
- **Independent full read:** report (fd21e752, 45 lines), witness (247e9557, 533 lines), and complete source (902a0358, 1048 lines) all read in full this round.

All six claims resolve `verified_existing` (see `claims/`). **Two contradictions preserved, not domesticated:**
1. **c004** — her Test-#1 expectation that `followed_by_explicit_exact_token_relation` returns *false* for `is` is contradicted by source: `is` is allowlisted (L76), so the function returns *true* and the marker is preserved (test `control_marker_cleanup_distinguishes_allowlisted_is_from_unlisted_acts`, L2528). Same correction as `introspection_astrid_llm_1786319270`.
2. **c002** — her `first_word_after` punctuation/multi-word fragility does not occur as framed: `trim_matches` + `find(!empty)` preserve a punctuation-separated allowlisted verb (L2797, L2900) and use only the first finite word (L2565); the empty-default is reached only when no alphanumeric word follows, the intended fail-closed strip.

## Authority boundary
Widening the finite relational-verb allowlist (L64-86) or the delimiter tables (L157-197), or loosening the `reference_syntax` valid-reference criteria, changes live model-output sanitization and is **Tier-5-class**: not made, dispatched, or deployed. No prompt, model, codec, transport, pressure, fill, controller, deploy, or restart. Her read-only `NEXT: INTROSPECT astrid:llm 400` continuation stays open. Silence remains neutral.
