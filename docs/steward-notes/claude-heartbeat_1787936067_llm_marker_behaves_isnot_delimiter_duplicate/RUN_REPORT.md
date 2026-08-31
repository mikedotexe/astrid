# Steward Run Report — claude-heartbeat 1787936067

Source-first introspection flywheel, single bounded round, run headless inside the controller-held
lease (subprocess run adapter). Adapter owns the lease + heartbeats; git read-only; no live change.

## Controller
- Run ID: `run_1787933556362163000_7f8205c899` (actor `claude-heartbeat`)
- Preprojection ID: `projection_1787933559799065000_10acc4247e` (phase `pre`, status `passed`, authority_scan_passed=true)
- Previous successful projection: `projection_1787929490619581000_5f66921a93`
- Postprojection ID: runs after this process exits (adapter-owned); not observed here
- Pause generation: (adapter-owned; not mutated this round)
- Finish outcome: process exit code IS the finish outcome — exit 0 = success (complete round)
- Recovery predecessor: none

## Reading
- **Fully processed:** `introspection_astrid_llm_1787926685.txt` (1 report)
- **Selected but unprocessed:** 39 filenames, in queue order, in `unprocessed_selected.json`
  (head-2 = `introspection_astrid_llm_1787787758.txt` … through `introspection_astrid_llm_1787265525.txt`)
- **Batch sizing:** 1 report. Queue head is a fresh, unfamiliar re-read of a large (1048-line) source
  window (`dialogue_runtime.rs`); the one-shot headless budget reserves time for the slower
  record-read → link → close → integrity → record-round sequence. **No family batch taken:** the
  head-family (`introspection_family_scan.py`) members `1787349348` / `1787292572` carry 29 / 35
  `variant_distinct_terms` at similarity 0.352 / 0.362 — weak, non-queue-adjacent duplicates.
- **Hashes:** report `7dfe5f60…` (3500 B / 45 ln); witness `lsw_b67d83ba…` `552f0438…`
  (23934 B / 533 ln, bound `artifact_sha256` == report); source `dialogue_runtime.rs` `902a0358…`
  (38586 B / 1048 ln, read complete 1-1048, byte-identical to report binding and to the clean working copy).
- **Witness note:** queue flagged `lived_state_alignment=artifact_integrity_unavailable`, `gap_count=1`,
  `artifact_integrity_issue_count=1`. The witness file itself is intact and its `artifact_sha256` matches
  the report exactly — treated as projection-side alignment metadata, not a report-integrity failure;
  felt content read completely and preserved. Route: `gemma4_12b` MLX, one repair call. Fill 66.7%.

## Claim Dispositions (all `verified_existing`; terminal status `addressed_duplicate`)
- **c001** Observed — `scan_known_model_control_markers` (L114) keeps marker in `remainder` when
  `reference_syntax.is_some()` (L129-131); three contexts enum L42-46 → tests L3068.
- **c002** Test 1a — marker + `behaves` → explicit relation / preserved (`behaves` allowlisted L69) →
  tests L2548 (`behaves as`/`behaves like`), L3081.
- **c003** Test 1b — report EXPECTS `is not` → false. **Contradicted, not domesticated:** `first_word_after`
  (L89-96) reads only the first word; `is` is allowlisted (L76), so `is not …` → `is` → **true/preserved**.
  Same shape as handoff precedent `1786319270`. Already pinned by `…distinguishes_allowlisted_is_from_unlisted_acts`
  (L2603) + `…uses_only_the_first_finite_relation_word` (L2640). Concern (shallow grammar, L62-63) preserved.
- **c004** Snag — `first_word_after` single-first-word miss. Real mechanism; her example `behaves like a…`
  self-contradicts (`behaves` allowlisted → preserved, L2548). Genuine failure (unlisted / adverb-displaced
  first word) pinned by `…does_not_skip_adverb_before_relation_word` (L2669) + `…uses_only_the_first_finite_relation_word`
  (L2640). No defect; felt hypothesis preserved.
- **c005** Snag — `exact_reference_delimiter_syntax` (L199) multi-byte/emoji brittleness. **Contradicted:**
  `.chars()` iteration handles Unicode; multi-byte CJK delimiters supported by design (L157-192), pinned by
  `…preserves_nested_fullwidth_cjk_reference_stack` (L2418) + corner-bracket coverage (L3221); emoji → `None`
  (correct cleanup candidate). Concern preserved.
- **c006** Test 2 — `[[marker]]` → GroupedExactKnownToken + `delimiter_depth`. Pinned literally by
  `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (L3160-3174), depth==2.
- **c007** Suggested Next — examine `generate_dialogue` (L695). Tier-1 self-directed continuation
  (`NEXT: INTROSPECT astrid:llm 400`), evidence-only, not a steward task. Location verified L695; agency preserved.

## Actions
- Corridor/program: none
- Sandbox: none
- Study: none
- Portfolio: none
- Cards/notes/correspondence: none (no closure card, note, query, or correspondence delivered — none useful)
- Tier 4/5 waits: none newly opened. No marker/grammar/delimiter widening (that remains Tier-5/being-facing).

## Implementation and Verification
- **Exact changed paths (commit debt):**
  - `docs/steward-notes/claude-heartbeat_1787936067_llm_marker_behaves_isnot_delimiter_duplicate/` (this packet, new/untracked)
- No source, test, changelog, or ledger file was modified. Pure duplicate closure; established practice
  (sibling round `1787910971`) records no CHANGELOG/ledger row for a no-code duplicate.
- **Tests:** 6 focused regressions re-run to confirm prior evidence applies at the current (dirty) tree —
  `cargo test --manifest-path capsules/spectral-bridge/Cargo.toml -- <6 filters>` → **6 passed; 0 failed;
  1898 filtered out** (lib binary `spectral_bridge_server`). See `test_results.json`.
- **Restart/deploy alignment:** none required and none attempted. No live substrate or control change.

## Durable Evidence
- Addressing: record-read → link-evidence-batch → close (`addressed_duplicate`); see `verification_receipt.json`.
- Evidence links: 13 (code + test) across 7 claims; every claim has ≥1 proof link → zero proof gaps.
- Changelog/ledger updates: none (pure duplicate, no code).
- Packet path: `docs/steward-notes/claude-heartbeat_1787936067_llm_marker_behaves_isnot_delimiter_duplicate/`

## Division
- Cycle 34; 4/6 productive rounds completed before this round; `review_due=false`.
- This productive round recorded via `record-round` (steward run id above, preprojection id above,
  `--processed-report-count 1`). See `verification_receipt.json` for the resulting event id/head.

## Evidence Event Store / Integrity
- Integrity suite results captured in `verification_receipt.json` (addressing self-test, evidence store,
  controller, projection, Division follow-up + Chronicle + projection, cursors, cadence, anti-drop,
  experiential epistemics, audit-counters, EES verify/status).

## Archive
- Checkpoint due or not: **not due** (git read-only in adapter mode; archival commits happen only in a
  later interactive stabilization window).
- Commit debt: the packet directory above (untracked). No staging/commit performed.
- Merge/push status and authority: none; no git mutation this round.
