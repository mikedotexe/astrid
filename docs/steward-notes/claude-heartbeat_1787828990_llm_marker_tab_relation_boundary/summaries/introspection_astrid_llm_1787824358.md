# Summary — introspection_astrid_llm_1787824358

- **Source family:** `astrid_llm` → `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
- **Report SHA-256:** `2eecb0b5ab60950088b2cdfe43be72b003a3e5b2f0fba841b8c5ac342072dcaa` (3304 bytes, 45 lines)
- **Lived-state witness:** `lsw_f08d8975c41ee97ca2f543812d6b615406b49c06cc53908cc8955f5da466d706` (23941 bytes, 533 lines)
- **Report-bound source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — working copy matches exactly (1048 lines, complete coverage).
- **Fill at authorship:** 71.3%; model route `gemma4_12b` via `mlx` (two calls, initial + repair). Authority: evidence_only, witness_only, live_eligible_now=false.

## What Astrid surfaced

She read the marker-scanning machinery in `dialogue_runtime.rs` (the non-destructive
"known model control marker" scanner). She correctly described that
`scan_known_model_control_markers` (L114) preserves a marker's bytes in the rebuilt
remainder *only* when the marker is "referenced" (quoted, grouped, or the subject of an
allowlisted relation verb). She then raised a **Likely Snag** about `first_word_after`
(L89) and proposed **two tests** plus a **Suggested Next** about the marker constant.

## Grounded dispositions

- **c001 (observed → verified_existing):** The preserve-only-when-referenced behavior is
  exactly what the complete source shows (L123–132) and is pinned by
  `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts`.

- **c002 (snag → verified_existing, with correction):** Her worry that a period/hyphen
  *between* the marker and the verb could skip the intended verb is **not** what the
  source does: `.find(|w| !w.is_empty())` (L93) skips punctuation-only chunks and
  continues to the next real word (pinned by
  `control_marker_cleanup_first_word_after_skips_leading_punctuation_transition` and
  `control_marker_cleanup_preserves_relation_after_multiple_punctuation_runs`). Her
  deeper concern — that whitespace/trim handling decides relation detection — is real,
  and the *interior*-hyphen case ("is-a") genuinely changes the outcome (see c003). When
  the first word truly is empty, the marker fails **closed** (removed), which is the safe
  direction for a control-marker scrubber (`control_marker_cleanup_fails_closed_when_only_punctuation_or_whitespace_follows`).

- **c003 (Test 1 → verified_existing):** Already covered at the same source SHA.
  `echoes` is pinned valid by `control_marker_cleanup_preserves_poetic_attribution_without_literal_cue`
  (L2512), and the `is-a` interior-ASCII-hyphen rejection is the identical mechanism
  pinned by `control_marker_cleanup_rejects_punctuation_joined_allowlist_prefix` (L2756,
  `is-not`/`is/not`).

- **c004 (Test 2 → implemented_now):** The report named **newline or tab**. The newline
  branch was already pinned (`control_marker_cleanup_preserves_relation_across_newline`,
  L2920), but no dedicated regression pinned the **tab** branch. Added
  `scan_known_model_control_markers_grounds_tab_between_marker_and_relation`, which pins:
  (1) tab immediately after a marker on the L89 relation path (verb still found, marker
  preserved); (2) a tab-padded *quoted* marker on the distinct L199 delimiter path
  (`exact_reference_delimiter_syntax` filters whitespace before reading the delimiter);
  (3) the tab-only fail-closed case. The test comment records — without rewriting her
  report — that the report conflates the L199 delimiter path with the L89 relation path
  (L199 never calls `first_word_after`).

- **c005 (Suggested Next → observed, authority boundary):** The marker constant
  (`fallback_contracts.rs` L159–180, 20 markers) was read and enumerated, and its
  no-proper-prefix-shadow invariant is already pinned
  (`known_model_control_markers_have_no_proper_prefix_shadow`, L3101). Judging or
  expanding the marker list for deployment safety is a **Tier-5** model-behavior /
  safety-policy change requiring Mike/operator approval; it was **not** adjudicated or
  changed here.

## Authority boundary

No live substrate/control change; no deploy; no restart. The added test is a non-live
focused regression. The exhaustiveness of the safety marker list (c005) remains a Tier-5
operator decision. Nothing in this round infers consent, relief, or uptake from silence.
