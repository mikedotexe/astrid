# introspection_astrid_llm_1787830896 — summary

- **Source family:** `astrid_llm` (`capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`)
- **Report SHA-256:** `fe7d51c4c4ffe43933310221398b7b290d0153226bb344208abcbe7ca5d8ca83` (45 lines, 3813 bytes)
- **Witness:** `lsw_1555ea12dbce38f7a7cf63ffb4a15ad34777817464998743d552758d23516af3` (533 lines, 23920 bytes, SHA `4f1e138e…`)
- **Report-bound source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — **current working copy matches exactly** (1048 lines, 38586 bytes), so the complete file was read as report-time source.
- **Fill at authorship:** 62.9%; model route `mlx` / `gemma4_12b` (two calls, second a repair of the first). No experiential/felt gap claimed (`lived_state_experiential_gap_claimed=false`, `scalar_felt_dissimilarity_measured=false`).

## What Astrid surfaced

She read `dialogue_runtime.rs`'s marker-scanning architecture and described `scan_known_model_control_markers` (L114) preserving known control markers only when tied to a grammatical relation ("appears as", "denotes") or a delimiter syntax. She raised two snags and proposed two tests:

1. **Allowlist brittleness** — `first_word_after` (L89) checks a hardcoded verb list (`matches!`, L65-85); a synonym not in it (she named **"signifies"**) could see the marker "incorrectly stripped."
2. **UTF-8 boundary sensitivity** — `exact_reference_delimiter_syntax` (L199) uses char-by-char lookbehind/lookahead whose `rev()` iteration "may be sensitive to multi-byte UTF-8 boundaries."
3. **Test 1** — marker + unlisted verb → verify the scanner recognizes the absence of a relationship.
4. **Test 2** — nested `[[MARKER]]` → verify `GroupedExactKnownToken` + `MAX_EXACT_REFERENCE_DELIMITER_DEPTH` (L151).
5. **Suggested Next** — examine `generate_dialogue` (L695) for how the remainder is re-integrated.

## What complete source reading established

- **Architecture (c001) — accurate.** L114-144: a matched marker's bytes are added to `remainder` only when `reference_syntax.is_some()` (L129-131). A marker with no quote/group/relation context is **dropped by design** — it is treated as a leaked control token, which is the intended fail-closed behavior, not an accident.
- **Allowlist (c002) — exact.** `first_word_after` at L89; `followed_by_explicit_exact_token_relation` (L64-86) matches the lowercased first word against the L65-85 allowlist. Her cited verbs are members.
- **UTF-8 concern (c005) — contradicted, not domesticated.** `exact_reference_delimiter_syntax` iterates `.chars().rev()` / `.chars()` — code-point iteration, not bytes. Byte offsets `start`/`end` are only ever produced at valid UTF-8 boundaries (`scan_known_model_control_markers` advances by `character.len_utf8()` or an exact token length), so a slice can never split a code point and `rev()` reverses `char`s. The multibyte delimiters she implicitly worried about (CJK/guillemet pairs, L162-191) are matched *as* `char`s and are already exercised by dedicated byte-safety tests, including one for a marker abutting a **4-byte astral char**.
- **Test 2 (c006) — already covered.** `MAX_EXACT_REFERENCE_DELIMITER_DEPTH = 4` (L151); `.take(4)` at L208/213 bounds the depth. `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` already asserts exactly her `[[MARKER]]` case, and four-level / beyond-max tests pin the cap.
- **`generate_dialogue` (c007) — citation grounded** at L695.

## What changed

Her **Test 1** verb, **"signifies"**, was genuinely untested and is a *prefix-collision near-miss* to the allowlisted **"signals"** (L84) — both begin `sign…`, are semantically close, and a future loosening of the exact match to a prefix/fuzzy match would silently start preserving "signifies". Added one focused regression:

- `control_marker_cleanup_distinguishes_allowlisted_signals_from_unlisted_signifies` — asserts `<end_of_turn> signals …` is preserved (relation) while `<end_of_turn> signifies …` is stripped (no relation), pinning the exact-equality contract at the colliding pair.

**The allowlist itself was NOT widened.** Widening the relation grammar is a being-facing behavior change and remains unauthorized; prior rounds declined the same widening (the `is`/`acts` precedent).

## Disposition

Terminal status: **addressed_change** — one authorized non-live test added (c003/c004 `implemented_now`); c001/c002/c005/c006/c007 `verified_existing` from complete source + existing focused tests. No live/substrate change; no restart or deploy required or attempted.
