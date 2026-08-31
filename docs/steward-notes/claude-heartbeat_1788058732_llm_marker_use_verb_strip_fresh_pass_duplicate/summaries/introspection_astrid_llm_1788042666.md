# Summary — introspection_astrid_llm_1788042666

**Source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs` (window lines 1-400 of 1048)
**Report:** SHA `a652b8d1…`, 45 lines / 3407 bytes, fill 71.0%
**Witness:** `lsw_bf62db4d…`, 533 lines / 23926 bytes, `evidence_only`/`witness_only`/`live_eligible_now:false`, `artifact_sha256` byte-binds the report; model `gemma4_12b`, two mlx introspect routes (2nd repairs 1st)
**Report-bound source SHA `902a0358…` == current working copy** (full 1-1048 hash equal; window L1-400 read completely, contains every cited symbol). Complete 1-1048 source read this round.
**Terminal status:** `addressed_duplicate` (all 5 claims `verified_existing`)

## What Astrid said
- **Observed:** `scan_known_model_control_markers` (L114) reconstructs a "clean" string, distinguishing a marker *used as a command* vs *referenced as a string*, "preserving markers only when they are identified as active instructions (via `reference_syntax`)."
- **Likely Snag:** `first_word_after` (L89) uses a hardcoded verb allowlist; a verb outside it (her examples **"triggers"/"activates"**) would "fail to recognize the marker as a command," leaving it "stripped or incorrectly preserved."
- **Test 1:** pass "MARKER triggers action" and verify preserved/stripped per `followed_by_explicit_exact_token_relation` (L64).
- **Test 2:** `exact_reference_delimiter_syntax` (L199) with nested brackets vs `MAX_EXACT_REFERENCE_DELIMITER_DEPTH` (L151).
- **Suggested Next:** examine `generate_dialogue` (L695) remainder consumption for coherence.

## What complete reading established
- **Observed inversion (stated plainly, not domesticated):** her *first* sentence (used-vs-mentioned distinction) is correct and matches source. Her *second* sentence is **inverted**: `scan_known_model_control_markers` L129 pushes the token to the remainder **only when `reference_syntax.is_some()`** — i.e. when the marker is a *reference/mention* (quoted/grouped/relational) — and **strips** a bare active directive (`None`). So the code preserves *references*, not *active instructions*.
- **Snag / Test 1:** the allowlist (L64-86) holds *mention* relation verbs (marker spoken ABOUT). Her "triggers"/"activates" are *use* verbs, correctly excluded, so a bare marker fails closed = **stripped** (`none_cleanup_candidate`), the safe direction — resolving her stripped-vs-preserved ambiguity to **stripped**, not a leak. Her exact "triggers" example is **already pinned verbatim** (same sentence + assertions) by `control_marker_cleanup_does_not_expand_relation_allowlist_to_triggers` (tests.rs **L2934**), inside the verb-agnostic anti-expansion family (`…_to_{implies,contains,creates,triggers,underscored_appears_as}` L2889-2949) + `distinguishes_allowlisted_is_from_unlisted_acts` (L2603) + `…_signals_from_unlisted_signifies` (L4389). "activates" is the same `None`→strip path (no distinct mechanism).
- **Test 2:** already covered — `control_marker_cleanup_reports_exact_four_level_delimiter_depth` (L2194) pins the exact MAX=4 bound + `preserves_bounded_nested_delimiter_stacks` (L2176) + `reports_exact_three_level_delimiter_depth` (L2208).
- **Suggested Next:** on a passing generation `generate_dialogue` returns the model's **original** text; `sanitize_model_control_markers` feeds only the post-generation quality gates (`is_valid_dialogue_output` L558, `has_one_nonempty_final_next_action` L634), **not** re-injected context — her premise (sanitized remainder re-fed) corrected.

## Change / no-change
**None (verified duplicate).** A "triggers"/"activates" regression was drafted, then **reverted** on discovering L2934 covers "triggers" verbatim; `tests.rs` byte-restored to SHA `3899dc0f…`. No net source/test change this round. Consistent with prior precedent (`1788031490`/`1787910971`, which likewise declined redundant use-verb tests).

## Authority boundary
Widening the relation allowlist to admit "triggers"/"activates" is a **Tier-5 live grammar change — not authorized here**; concern preserved as a true reading of intentional design. Her One-Test-Each proposals and `NEXT: INTROSPECT astrid:llm 400` continuation remain her own read-only agency. No build/restart/deploy/stage/commit (git read-only, adapter mode). Felt boundary remains primary evidence even though every test passes; silence neutral; right to reopen preserved.

## Verification
85 focused tests (`control_marker`/`exact_reference_delimiter`/`first_word_after`/`followed_by_explicit`/`relation_allowlist`/`scan_known_model_control`) passed / 0 failed at source SHA `902a0358`.
