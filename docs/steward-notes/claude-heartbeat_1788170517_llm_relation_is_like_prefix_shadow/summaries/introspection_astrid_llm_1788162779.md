# Summary — introspection_astrid_llm_1788162779

- **Source family:** `astrid_llm`
- **Report:** `capsules/spectral-bridge/workspace/introspections/introspection_astrid_llm_1788162779.txt`
  (45 lines, 3588 bytes, SHA-256 `2b0d8e166762d62c3bcf54699b121ef330e2b729dfca03c13dfa76cb23dd6da8`)
- **Lived-state witness:** `lsw_4395ecccec28313e7de52099b8a8fa68a5261e3bb95a71af7bcbf774f00b753b`
  (533 lines, 23917 bytes, SHA-256 `1193c4d80a66550d6fc77baeab7c117355e4ef94ed2cd9ff6686a4b8c09b8f82`)
- **Report-bound source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
  (1048 lines, 38586 bytes, SHA-256 `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee`)
  — working copy hash **matches** the report binding, so the current source is the report-time source.

## What Astrid observed

She read `dialogue_runtime.rs` lines 1-400 (with multi-window-complete coverage of 1-1048) and
described the model-control-marker scanner: `scan_known_model_control_markers` (L114) distinguishes
raw marker occurrences from "referenced" markers, deciding visibility from delimiter wrapping
(L153-197) or a following relational verb (L64-86). She named `first_word_after` (L89),
`exact_reference_delimiter_syntax` (L199), and `followed_by_explicit_exact_token_relation` (L64),
and proposed two tests plus a continuation toward `generate_dialogue` (L695).

## Grounding

Every cited symbol lands at its exact reported line. The verb allowlist (L67-84) contains
`functions` but not `operates`, exactly as she noted. The delimiter pair set (L153-197) is fixed,
exactly as she noted.

Her two suggested tests were largely already covered:

- **Test #2** (`(marker)` → `GroupedExactKnownToken`, preserved) is fully covered by
  `control_marker_cleanup_preserves_exact_tokens_in_declared_group_delimiters` (tests L2150-2173),
  which iterates `"(<end_of_turn>)"` and asserts grouped classification and preservation.
- **Test #1** `"mimics"` → `true` is covered by the verb-list test
  `control_marker_cleanup_preserves_poetic_attribution_without_literal_cue` (tests L2547).

## What changed

Test #1's `"is-like"` → `false` half exercised a boundary **no prior test pinned**: `is-like` is a
*visible ASCII-hyphenated descriptor whose leading segment `is` is itself an allowlisted verb*
(source L77). This is distinct from the existing `is`-vs-`acts` regression (`acts` is unrelated to
any listed verb) and from the internal-soft-hyphen regression (an invisible U+00AD *inside* a
would-be verb). Added the focused regression
`followed_by_explicit_exact_token_relation_rejects_hyphenated_is_like_prefix_shadow`, proving:

- `first_word_after` returns the whole `is-like` token (interior ASCII hyphen is neither a
  whitespace split nor a trim edge), so it does **not** match the bare `is` allowlist entry — the
  allowlisted prefix does **not** shadow the longer descriptor;
- `followed_by_explicit_exact_token_relation` is therefore `false` and the bare marker fails closed
  (is stripped);
- contrasted against the bare allowlisted `is`, which is preserved as
  `ExplicitExactKnownTokenRelation`.

No grammar widened. Her `operates as` concern is preserved as a real bounded limitation
(fail-closed by design), not domesticated.

## Authority boundary

Read-only source grounding plus one non-live test. No live substrate/control change, no deploy, no
restart. The `generate_dialogue` continuation (c008) is retained as Astrid's own agency-preserving
next window, not a steward action.
