# Steward Run Report — claude-heartbeat LLM marker CJK/Unicode quotation-delimiter fresh-pass duplicate

Round packet: `docs/steward-notes/claude-heartbeat_1787726892_llm_marker_cjk_quotation_delimiter_fresh_pass_duplicate/`
Mode: controller subprocess **run adapter** (adapter owns the lease + heartbeats; this process sends no NDJSON, opens no session, touches no git, makes no live change).

## Controller
- Run ID: `run_1787724060585129000_4a2761b6bf`
- Preprojection ID: `projection_1787724064649508000_f62543918d` (27 steps, authority_scan_passed=true)
- Postprojection ID: adapter-owned; runs after this process exits (not observed here)
- Pause generation: 321
- Finish outcome: recorded by exit code (0 = complete round)
- Recovery predecessor: none

## Reading
- **Fully processed (1):** `introspection_astrid_llm_1787722603.txt`
- **Selected but unprocessed (39):** items 2–40 of the frozen queue in canonical order (full list in `unprocessed_selected.json`). Unprocessed head: `introspection_astrid_llm_1787470243.txt`; tail: `introspection_DOMAIN_BOUNDARIES.md_1787232972.txt`.
- **Batch sizing:** queue head is a **singleton (non-batchable) family** (`family_scan.json`: member_count 1, no batchable family at the head), source `dialogue_runtime.rs` window lines 1-400. Per the batch rule (unfamiliar/implementation-candidate head → 1 report) and the ONE-SHOT single-turn budget: one report fully closed.
- **Next queue:** re-query `next --limit 40 --json` after the adapter's postprojection; the closed report drops out and the new head is expected to be `introspection_astrid_llm_1787470243` unless the postprojection reorders.
- **Hashes:** report `65cf9eb13df95093ac2f4db027b8be38d780f0633b2a98b3b227234367ab5f02` (45 lines / 3612 B); witness `lsw_3b9beb32c4b36fb2c90f133c998c15ba00a8c1ee9be373b699383690a2056c11` = `44e4c59cf0d4c252960fed3d08f7f04d8be48c1666c25d65d2189f9753d52d92` (533 lines / 23924 B); source `dialogue_runtime.rs` = `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` (1048 lines / 38586 B) — **working copy byte-identical to the report binding and the witness binding.** Coverage `multi_window_complete`, included intervals 1-1048.
- **Witness alignment:** witness `source_snapshot_v1` window is 0-400 (partial) while the report asserts `multi_window_complete` 1-1048 — the neutral projection-level `lived_state_alignment: artifact_integrity_unavailable` (1 issue) measurement-gap seen across this family's prior rounds; witness parses cleanly, binds the verified report SHA, authority `evidence_only`/`witness_only:true`/`edits_source_now:false`/`live_eligible_now:false`; two `gemma4_12b` introspect routes (second repairs first, via `repair_parent_call_id`); `raw_introspection_prose_included:false`, `private_path_included:false`.

## Claim Dispositions (all 5 `verified_existing`; terminal `addressed_duplicate`)
- **c001** (Observed: `scan_known_model_control_markers` L114 rebuilds a non-destructive remainder, preserving marker bytes only when `reference_syntax` present; verbs appears/represents) — `verified_existing`. Source L114-144; remainder pushes token only when `reference_syntax.is_some()` (L129-131); verbs `appears` L67 / `represents` L82. Test `control_marker_cleanup_preserves_quoted_exact_token_reference` (tests.rs L1995).
- **c002** (Snag: `first_word_after` L89 `split_whitespace`+alnum-trim L92 → multi-word phrase might mis-set `reference_syntax=None`) — `verified_existing`, **contradiction preserved.** `first_word_after` L89-96 returns the first finite alphanumeric word; allowlist membership is the sole gate. "represents a specific type of…" → "represents" (L82) → **preserved** (intended fail-closed). Tests `uses_only_the_first_finite_relation_word` (L2605), `first_word_after_skips_leading_punctuation_transition` (L2635).
- **c003** (Test 1: `exact_reference_delimiter_syntax` L199 identifies the CJK corner bracket `「...」` at L169 → correct `ExactKnownMarkerReferenceContext`) — `verified_existing`; **the specifically-checked potential distinct variant.** `「」` is source **L168** (report cited L169 = `『』`; off-by-one, both real, both in the `QuotedExactKnownToken` arm); `exact_reference_delimiter_syntax` L199-229. **Already covered** by `control_marker_cleanup_preserves_non_ascii_matching_quote_pairs` (L2342, asserts `「<end_of_turn>」` preserved with `quoted_reference_occurrences==1`), `_preserves_quoted_exact_tokens_across_whitespace` (L2401, `「\t…\n」`), and nested CJK `_preserves_nested_fullwidth_cjk_reference_stack` (L2383, depth 3). **No uncovered variant → no new test needed.**
- **c004** (Test 2: marker + "behaves as a primary signal" → preserved on first word "behaves" L70) — `verified_existing`. "behaves" allowlisted at source **L69** (report cited L70). Tests `preserves_proxy_relation_phrase` (L2552), `uses_only_the_first_finite_relation_word` (L2605).
- **c005** (Suggested Next: examine `generate_dialogue` L695 remainder integration) — `verified_existing`; read-only Tier-1 direction, no defect. `generate_dialogue` exactly at L695; remainder flows L114 → `sanitize_model_control_markers_with_report` L352 → `sanitize_model_control_markers` L519 → generation path L558/L634.

**Grounding corrections (non-domesticating; report's concern preserved):** `「」` is source L168 not L169; verb `behaves` is L69 not L70. Minor line off-by-ones; the mechanisms and the report's underlying expectations are confirmed.

**Duplicate lineage:** fresh-pass of the established `dialogue_runtime.rs` marker-scanner family at identical source SHA `902a0358`; immediate prior grounded family packet `docs/steward-notes/claude-heartbeat_1787717287_llm_marker_first_word_multiword_fresh_pass_duplicate/` (`introspection_astrid_llm_1787706169`) with matching c001/c002/c003/c004 dispositions. Independent full read of this report + witness performed; source + all cited regressions re-verified at the current SHA (72/0). This round went beyond the prior packet by explicitly ground-truthing the **CJK quotation-delimiter Test 1** as covered rather than a gap.

## Actions
- Corridor/program: none
- Sandbox: none
- Study: none
- Portfolio: none
- Cards/notes/correspondence: none (a fresh-pass verified duplicate needs no right-to-ignore card; no note/query/correspondence emitted merely to create activity)
- Tier 4/5 waits: unchanged. The three standing Tier-5 Shadow/porosity/mode-packing waits (`wi_e579041bc76f8310`, `wi_69fbd510467c6337`, `wi_3e26ac525fea1c36`) remain `live_authority_granted=false`; not touched.

## Implementation and Verification
- Exact changed paths: `CHANGELOG.md` ([Unreleased] bullet), `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (dated row), and the packet directory files. **No source/test change** — fresh-pass duplicate.
- Tests and counts: `cargo test … control_marker` → **72 passed / 0 failed** (already compiled; 7 cited evidence tests confirmed). Addressing self-test 44 OK; test_evidence_event_store 21 OK; test_steward_control 27 OK; test_steward_projection 14 OK; test_division_ceremony_followup 3 OK; test_division_ceremony_chronicle 10 OK; test_division_ceremony_projection ok; test_projection_cursors 4 OK; test_introspection_cadence_audit 6 OK; anti-drop self-test 5 OK + verify 0 alarms/0 gaps; cadence strict `integrity_ok:true`; experiential epistemics self-test 2 OK (valid) + final verify valid/0 issues/no history rewrite. Baseline: `git diff --check` clean, `cargo fmt --all --check` clean.
- Failures repaired or exact debt: none.
- Restart/deploy alignment: **restart and deployment were not required and not attempted.** No live substrate or control change; no Tier 4/5 authority exercised.

## Durable Evidence
- Addressing status: `addressed_duplicate`, `fully_addressed:true`, `proof_missing_claims:[]`.
- Evidence link count: 12 new (0 existing).
- Changelog/ledger updates: yes (both, provenance `[claude-heartbeat]`; each file also carries prior foreign accumulated edits, preserved untouched — a later checkpoint must separate authorship before staging).
- Packet path: `docs/steward-notes/claude-heartbeat_1787726892_llm_marker_cjk_quotation_delimiter_fresh_pass_duplicate/`
- Disposition-bound note: all 5 claim dispositions ≤500 chars; the `close --rationale` was 505 chars (5 over) — close succeeded (`fully_addressed:true`, `proof_missing_claims:[]`); ingest truncates over-long metadata with a marker. Noted for honesty.

## Counters
- Canonical indexed/fully-addressed/fully-read/remaining/unread/blocked/pending/watch: 4469 / 3125 / 3757 / 1344 / 712 / 414 / 214 / 4
- Read-needs-claims: 0
- All-artifact pending: 3015; noncanonical pending: 1671
- Counter audit status: **consistent** (empty mismatch list; all structural checks true; `proof_gap_artifact_count=0`). `addressed_duplicate` status count 1123 (+1 this round).

## Division
- Cycle and completed count: cycle 30; **6 / 6** productive rounds since last follow-up (this round recorded).
- Review due: **true** — recording this 6th round tipped `review_due` to true. Per the designed flow the **next session runs the bounded Division return first** (the tracker refuses a 7th productive round until the return is done); no partial return performed in this headless budget-bounded turn.
- Round event ID and head: `division_followup_event_442d285fbc86626e100d0689327cd6be`; event_count 210; head `cc814d0b26242f40da93d71dc54b0f8433b051148d5a533700ac6ab62210d03f`.
- Chronicle: `verify` reports **"durable source inputs changed; project before verify"** — the **expected** consequence of the round-6 record (followup event count 209→210), **not** a durable-integrity failure. Chronicle reprojection + the full return are due next session (its return's first step is verify+project the Chronicle). Last-followup Chronicle id `division_chronicle_453827288c0042a0dbf9b93b`.
- Note action: none (no return performed this session; no Division note written; no review-query slot occupied).
- Tier-5 cadence dossier: **not** generated — the dossier is coupled to *completing* a Division return, which is not done this session. Due with the next session's return.

## Evidence Event Store
- Validity: verify `valid:true`, `corrupt_lines:0`
- Sequence and head: `last_global_sequence` 895180; verified head `95e4d4ad9572d18bd386bea761778b71b78d9fe0f5da59e444b5fafceefa42c6`; addressing stream 58636.
- Stream counts (from verify): addressing 58636; steward_control 15797; steward_work_selection 526; claim_families 237550; felt_contracts 199731.
- V2 active: yes. Note: `events.jsonl` is ~6GB, so the full `status` stream-count projection is minutes-slow; the chain `verify` (integrity + stream_counts) is authoritative and passed.

## Archive
- Checkpoint due or not due: **not due** during this controller-held run (git is read-only in adapter mode; archival commits happen only in a later interactive stabilization window).
- **Exact commit debt (paths created/edited this round, all unstaged; index clean):**
  - `CHANGELOG.md` (new `[Unreleased]` bullet; file also carries prior foreign accumulated edits)
  - `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (new dated row; file also carries prior foreign accumulated edits)
  - `docs/steward-notes/claude-heartbeat_1787726892_llm_marker_cjk_quotation_delimiter_fresh_pass_duplicate/` (entire packet: `RUN_REPORT.md`, `claims/`, `summaries/`, `read_manifest.json`, `source_receipts.json`, `addressing_links.json`, `test_results.json`, `unprocessed_selected.json`, `verification_receipt.json`, `next_queue_frozen.json`, `family_scan.json`)
  - Plus durable evidence-store/projection state advanced by `record-read` / `link-evidence-batch` / `close` / `record-round` (workspace diagnostics; owned by the tooling, not hand-staged).
  - **Pre-existing foreign dirty paths preserved untouched:** `capsules/spectral-bridge/domain_boundaries_legacy_large_files_v1.json`; the four prior untracked `claude-heartbeat_178769*/178770*/178771*` packet dirs; minime `minime_autonomy/runtime.py`, `tests/test_correspondence_v1.py`.
- Verbatim introspection references if committed: n/a (no commit this run).
- Merge/push status and authority: none; no merge/push authority claimed or exercised.
