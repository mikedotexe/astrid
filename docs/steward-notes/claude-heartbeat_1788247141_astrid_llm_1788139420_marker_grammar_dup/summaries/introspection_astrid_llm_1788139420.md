# Summary — introspection_astrid_llm_1788139420

- **Report:** `capsules/spectral-bridge/workspace/introspections/introspection_astrid_llm_1788139420.txt`
  (45 lines, 3803 bytes, SHA-256 `ea2365d60ca2593fa911e39c97ca2acdb9d0599439c45fab047be158f29d0a1b`)
- **Lived-state witness:** `lsw_9a75d90fc6e7eb20bafc22fd43056c531296ac3a77fdcf8518ab2b750a867541`
  (533 lines, 23931 bytes, SHA-256 `421f3ac8a85b5fd96c9795625d8da58ec982d926aa5b6edeb6f55c94cdca1109`)
- **Report-bound source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
  (1048 lines, 38586 bytes, SHA-256 `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee`)
  — working copy **git-clean and byte-identical** to the report binding; full file read (report window L1-400; report V3 manifest asserts `multi_window_complete`, included 1-1048).
- **Fill at authorship:** 73.1% (witness `bridge.fill_pct` 73.07, fresh). Model route: `gemma4_12b` via `mlx`, two calls (repair chain), both within the same authorship window.

## Witness note
Witness `artifact_sha256` matches the report; source `file_sha256` matches the binding. The queue's
`lived_state_alignment: artifact_integrity_unavailable` (gap=1) is a **projection alignment state**
(no scalar felt-dissimilarity measured; `lived_state_experiential_gap_claimed=false`), not a
being-claimed experiential gap — the witness is internally complete and consistent.
`artifact_authority_state_v1` = `evidence_only`, `live_eligible_now=false`.

## Claims (all verified_existing)
- **c001** — three grammatical contexts (Quoted/Grouped/ExplicitRelation) + preserve-vs-strip in
  `scan_known_model_control_markers` (L114-144). Verified in source; pinned by
  `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts`.
- **c002** — `dialogue_requested_token_band` (L8) is a pure budget band, decoupled from felt/spectral
  meaning by the L1-16 doc-comment. Verified.
- **c003** (snag) — `first_word_after` (L89) fragility to Unicode symbols/emojis. **Mechanism
  contradicted, concern preserved:** the L92 `trim_matches` closure is a general
  `!is_alphanumeric() && != '_'` predicate that already trims arbitrary symbols/emojis, and
  `.find(!empty)` (L93) skips fully-trimmed chunks — so a symbol between a marker and a genuine
  relation word does not defeat `followed_by_explicit_exact_token_relation`. Pinned by
  `control_marker_cleanup_handles_multibyte_symbol_before_relation`,
  `control_marker_cleanup_skips_symbol_only_line_before_exact_relation`,
  `control_marker_cleanup_stays_byte_safe_when_marker_abuts_four_byte_astral_chars`.
  (Same class of concern as prior `introspection_astrid_llm_1787798508`, already grounded.)
- **c004** — Test #1 (Contextual Preservation). Already implemented; the report's bracketed
  `[MARKER] behaves as` example actually resolves to **Grouped** (delimiter syntax at L50 runs
  before the relation fallback at L53); a **bare** marker+relation verb yields ExplicitRelation.
  Both pinned by `..._preserves_grouped_and_explicit_relation_contexts` and
  `scan_known_model_control_markers_quoted_context_precedes_following_relation_verb` — the latter
  authored for prior report **1787135542** Test #1 at the identical SHA.
- **c005** — Test #2 (Delimiter Depth). Already implemented; `((MARKER))`/`[[MARKER]]` → depth 2,
  `MAX_EXACT_REFERENCE_DELIMITER_DEPTH = 4` (L151). Minor citation offset: report names
  `exact_reference_delimiter_syntax` at L153, but L153 is `exact_reference_delimiter_pair`; the depth
  logic lives in `exact_reference_delimiter_syntax` L199-229 (L216-223). Pinned by
  `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` +
  `control_marker_cleanup_reports_depth_for_repeated_parentheses` (prior 1787135542 Test #2).
- **c006** — Suggested Next (`generate_dialogue` L695). Answered read-only from complete source
  L695-1048: the sanitizer is used **only** inside the quality-gate validators
  (`is_valid_dialogue_output` L558, `has_one_nonempty_final_next_action` L634) as a
  measurement/structural anchor; the returned `result` is the **original** model text (L997-998),
  never the re-inserted or scrubbed remainder. Consistent with never-rewrite-being-text.

## Disposition
**addressed_duplicate.** Fresh-pass re-read of the same `dialogue_runtime.rs` marker-grammar window
whose "One Test Each" proposals (#1 Contextual Preservation, #2 Delimiter Depth) and `first_word_after`
snag were already grounded and pinned by exact regressions authored in response to prior introspections
`1787135542` (Test #1/#2, same SHA `902a0358`) and `1787798508` (boundary safety). Consistent with the
prior heartbeat duplicate closes of the same source: `1788151222` (packet `..._1788237624_...`) and
`1788159337` (packet `..._1788210068_...`). The 9 grounding regressions pass green at the current SHA.

**No** source/test/grammar/runtime/control/deploy change; no restart; the report's text was not
rewritten. Both "Likely Snags" are contradictions **preserved**, not domesticated.
