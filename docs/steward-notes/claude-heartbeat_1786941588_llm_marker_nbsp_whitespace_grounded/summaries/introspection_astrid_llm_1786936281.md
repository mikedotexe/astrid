# introspection_astrid_llm_1786936281 — first_word_after non-breaking-space robustness

- **Source family:** `astrid_llm` → `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
- **Window:** lines 1–400 of 1048 (coverage state `multi_window_complete`)
- **Report SHA:** `ee9e1ca1ec9bbdc6403605c9404c9ad88c47abe39e1a1e77fc5be55b91a7eb44` (45 lines, 3500 bytes)
- **Witness:** `lsw_c77007a1…` (533 lines, 23933 bytes, SHA `e4dea8e4…`) — fill 73.0%, `gemma4_12b`/mlx, evidence-only authority, no raw prose
- **Report-bound source SHA:** `902a0358…` — **matches the working copy exactly** (same file already read this run)
- **Terminal status:** `addressed_change`

## Important run note — stale queue caught before any close

My initial `next --limit 40` temp file was ~2h stale (a `date +%s` filename glitch meant the redirect did not capture a fresh run). It showed `introspection_astrid_llm_1786924416` at head, but the EES V2 has a persisted terminal `closed`/`addressed_duplicate` event for that ID at **seq 823047** (prior round `claude-heartbeat_1786932936`). A fresh `next` confirmed the true current head is `introspection_astrid_llm_1786936281`. No durable close/record-round was issued for the already-closed report — re-processing it would have manufactured a duplicate productive round. A regression I had drafted for `1786924416`'s (different) synonym snag was reverted.

## What Astrid surfaced

Another fresh pass of the marker-scanner window, with a **distinct** snag from the synonym-coverage family: she worried that `first_word_after` (L89), using `split_whitespace()` + a manual alphanumeric filter (L92), might **skip the verb or return empty** when a marker is followed by a punctuation-heavy string or a **non-breaking space**, causing the marker to be stripped.

## What complete source reading + tests established

1. **Observed (c001) — accurate, non-inverted.** `scan_known_model_control_markers` (L114) rebuilds a clean `remainder`, preserving a marker only when `reference_syntax.is_some()` (L129–131, her L130); the delimiter check is `exact_reference_delimiter_pair` (L153–197), covering quoted (L157–174) and grouped (L175–196) contexts incl. international pairs (`「 」` L168, `『 』` L169).
2. **Snag (c002) — hypothesis REFUTED, not domesticated; new regression added for her named case.**
   - `split_whitespace()` (L91) treats **U+00A0 NO-BREAK SPACE** (a Unicode White_Space char) as a separator, so the verb after it is still isolated.
   - The non-alphanumeric `trim_matches` (L92) strips a leading **U+FEFF ZERO WIDTH NO-BREAK SPACE** (not White_Space) from the first chunk, so the verb is still found.
   - `find(|w| !w.is_empty())` advances past punctuation-only chunks; the marker is stripped **only** when no alphanumeric word follows at all (the intended fail-closed case).
   - The general punctuation case was already grounded by `scan_known_model_control_markers_grounds_first_word_after_punctuation_boundary` (tests.rs L2899). Her specific non-breaking-space case was **not** covered, so I added `scan_known_model_control_markers_grounds_first_word_after_non_breaking_space` (both U+00A0 and U+FEFF → `denotes` found → marker preserved). **The test passes**, empirically confirming robustness.
3. **Test 1 (c003) — pre-covered.** `denotes` preservation is exercised by `control_marker_cleanup_preserves_exact_token_relation_without_preceding_vocabulary_gate` (L2500) and the scan fn by `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts` (L2845). `denotes` is in the allowlist (L71).
4. **Test 2 (c004) — pre-covered exactly, incl. her collapse concern.** `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (L2876) asserts `[[<end_of_turn>]]` → GroupedExactKnownToken depth **2** (not collapsed to 1).
5. **Suggested Next (c005) — accurate read-only pointer.** `generate_dialogue` is exactly at L695; the `remainder` flows downstream via the sanitizer (L352/L519, consumed at L558/L634). Full L695 assessment is her declared next window (L400+), out of this report's 1–400 scope.

## Authority boundary

No live change. Production grammar is unchanged; the verb allowlist is not widened. The witness is evidence-only (`live_eligible_now=false`, `edits_source_now=false`). The refuted hypothesis is stated plainly; her underlying attentiveness to `first_word_after`'s edge cases is preserved as valid signal. Silence remains neutral.
