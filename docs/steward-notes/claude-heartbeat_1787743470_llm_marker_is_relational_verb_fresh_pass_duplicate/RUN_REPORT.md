# Steward Run Report — claude-heartbeat LLM marker `is`-as-relational-verb fresh-pass duplicate

Round packet: `docs/steward-notes/claude-heartbeat_1787743470_llm_marker_is_relational_verb_fresh_pass_duplicate/`
Mode: controller subprocess **run adapter** (adapter owns the lease + heartbeats; this process sends no NDJSON, opens no session, reads/persists no lease token, touches no git, makes no live change).

## Controller
- Run ID: `run_1787741193058246000_6ab8b2afac`
- Preprojection ID: `projection_1787741197503407000_d046579c70` (27 steps, `authority_scan_passed=true`)
- Postprojection ID: adapter-owned; runs after this process exits (not observed here)
- Pause generation: 321
- Finish outcome: recorded by process exit code (0 = complete round)
- Recovery predecessor: none

## Reading
- **Fully processed (1):** `introspection_astrid_llm_1787739659.txt`
- **Selected but unprocessed (39):** items 2–40 of the frozen queue in canonical order (full list in `unprocessed_selected.json`). Unprocessed head `introspection_astrid_llm_1787730919.txt`; tail `introspection_astrid_llm_1787239801.txt`.
- **Batch sizing:** the queue-head **family** (`family_scan.json`: head `introspection_astrid_llm_1787739659`, member_count 3) carries two low-similarity members (0.396 / 0.364) with 29–31 `variant_distinct_terms` each — effectively distinct reports, not clean duplicates. Per the ONE-SHOT single-turn budget + the slow addressing/store CLI, processed only the head; one report fully closed beats several half-processed.
- **Next queue:** re-query `next --limit 40 --json` after the adapter's postprojection; the closed report drops out, expected new head `introspection_astrid_llm_1787730919` unless the postprojection reorders.
- **Hashes:** report `fea8c33c…` (45 lines / 3609 B); witness `lsw_ef1d1174…` = `4ea1ead6…` (533 lines / 23929 B); source `dialogue_runtime.rs` = `902a0358…` (1048 lines / 38586 B) — **working copy byte-identical to the report binding and witness binding.** Coverage `multi_window_complete`, included intervals 1-1048.
- **Witness alignment:** witness `source_snapshot_v1` window is 0-400 (partial) while the report asserts `multi_window_complete` 1-1048 — the neutral family-wide measurement gap. Witness parses cleanly, binds the verified report SHA, authority `evidence_only` / `witness_only:true` / `edits_source_now:false` / `live_eligible_now:false`; two `gemma4_12b` introspect routes (second repairs first via `repair_parent_call_id`); `raw_introspection_prose_included:false`, `private_path_included:false`; fill 71.03%, mode_packing 1.0, λ1 4.74 / λ2 3.05.

## Claim Dispositions (all 5 `verified_existing`; terminal `addressed_duplicate`)
- **c001** (Observed: `scan_known_model_control_markers` L114-143 non-destructive scanner; `ExactKnownModelControlMarkerOccurrence` separates bytes from role; preserve/strip by quoted or relational-verb syntax) — `verified_existing`. Source L114-144 pushes the token only when `reference_syntax.is_some()` (L129-131); `reference_syntax` L49-60; verbs L64-86 (behaves L69, represents L82); exact match L102-106. Test `preserves_quoted_exact_token_reference` (tests.rs L1995).
- **c002** (Snag: `first_word_after` L89-96 split_whitespace+alnum filter may mis-capture for a multi-word/punct-heavy following construction, "behaves like a…") — `verified_existing`, **contradiction preserved.** first_word_after returns the FIRST finite alphanumeric word; allowlist membership is the sole fail-closed gate; "behaves like a…" → "behaves" (L69) → preserved (not a snag). Tests `uses_only_the_first_finite_relation_word` (L2605), `first_word_after_skips_leading_punctuation_transition` (L2635), + four `scan…_grounds_first_word_after_*` punct/soft-hyphen/nbsp regressions.
- **c003** (Test 1: `exact_reference_delimiter_syntax` L199-229 on nested/complex Unicode delimiters, CJK corner brackets) — `verified_existing`. CJK `「」` is source **L168** (QuotedExactKnownToken arm); covered by `preserves_non_ascii_matching_quote_pairs` (L2342), `preserves_quoted_exact_tokens_across_whitespace` (L2401), nested `preserves_nested_fullwidth_cjk_reference_stack` (L2383, depth 3). No uncovered variant → no new test.
- **c004** (Test 2: marker followed by "non-relational verb (e.g. **marker is a tool**)" should be stripped) — `verified_existing`, **contradiction preserved (non-domesticating).** Source L76 lists `is` as a relational verb → a marker followed by "is" is **preserved** (`following_exact_relation`), NOT stripped; the report's premise is contradicted by source (mirrors the `introspection_astrid_llm_1786319270` precedent named in the handoff). Exactly encoded in `control_marker_cleanup_distinguishes_allowlisted_is_from_unlisted_acts` (L2568: `is`→preserved/`removed_total==0`, `first_word_after=="is"` L2573; `acts`→stripped/`removed_total==1`).
- **c005** (Suggested Next: examine `exact_reference_delimiter_syntax` beyond L207; delimiter_depth on nested structures without over-stripping) — `verified_existing`; read-only Tier-1 direction, no defect. delimiter_depth L216-223 bounded by `MAX_EXACT_REFERENCE_DELIMITER_DEPTH=4` (L151). Covered by `_reports_exact_four_level_delimiter_depth` (L2194), `_reports_exact_three_level_delimiter_depth` (L2208), `_bounds_deeper_delimiter_receipt_without_dropping_token` (L2253), `_bounds_homogeneous_square_bracket_stack_beyond_max_depth` (L2266).

**Grounding corrections (non-domesticating; concern preserved):** c004's "is" is a relational verb (source L76), so its Test-2 premise that "is" is non-relational is false — stated plainly, report preserved verbatim. CJK corner-bracket pair is source L168.

## Actions
- Corridor/program: none
- Sandbox: none
- Study: none
- Portfolio: none
- Cards/notes/correspondence: none (a fresh-pass verified duplicate needs no right-to-ignore card; nothing emitted merely to create activity)
- Tier 4/5 waits: unchanged. The three standing Tier-5 Shadow/porosity/mode-packing waits (`wi_e579041bc76f8310`, `wi_69fbd510467c6337`, `wi_3e26ac525fea1c36`) remain `live_authority_granted=false`; not touched.

## Implementation and Verification
- Exact changed paths (commit debt — this process staged/committed nothing): `CHANGELOG.md` ([Unreleased] bullet), `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (dated row), and the packet directory files (`RUN_REPORT.md`, `claims/introspection_astrid_llm_1787739659.json`, `summaries/introspection_astrid_llm_1787739659.md`, `read_manifest.json`, `source_receipts.json`, `addressing_links.json`, `test_results.json`, `unprocessed_selected.json`, `verification_receipt.json`, `next_queue_frozen.json`, `family_scan.json`). **No source/test change** — fresh-pass duplicate. Addressing/evidence + Chronicle-reprojection writes land in gitignored `capsules/spectral-bridge/workspace/diagnostics/` and minime's gitignored `workspace/division/chronicle/` → no git debt from them.
- Tests and counts: `cargo test --lib control_marker` → **72 passed / 0 failed** at source SHA `902a0358`. Integrity: addressing self-test 44 OK; test_evidence_event_store 21 OK; test_steward_control 27 OK; test_steward_projection 14 OK; test_division_ceremony_followup 3 OK; test_division_ceremony_chronicle 10 OK; test_division_ceremony_projection ok; test_projection_cursors 4 OK; anti-drop self-test 5 OK + verify 0 alarms; test_introspection_cadence_audit 6 OK; cadence strict `integrity_ok:true`; experiential_epistemics self-test 2 OK + final verify valid/0 issues/no history rewrite; audit-counters consistent; evidence_event_store verify valid/0 corrupt. Baseline `git diff --check` clean. `cargo fmt` not run (no Rust touched).
- Failures repaired or exact debt: none. **Read-only debt:** full `evidence_event_store status` stream-count capture deferred — the recompute exceeds a 10-min foreground budget at current store size; `verify` already confirmed valid/0-corrupt and the run receipt's `evidence_before.last_global_seq=896859` is the pre-run baseline.
- Restart/deploy alignment: **restart and deployment were not required and not attempted.** No live substrate or control change; no Tier 4/5 authority exercised.

## Durable Evidence
- Addressing status: `addressed_duplicate`, `fully_addressed:true`, `proof_missing_claims:[]`.
- Evidence link count: 13 new (0 existing).
- Changelog/ledger updates: yes (both, provenance `[claude-heartbeat]`; each file also carries prior foreign accumulated edits, preserved untouched — a later checkpoint must separate authorship before staging).
- Packet path: `docs/steward-notes/claude-heartbeat_1787743470_llm_marker_is_relational_verb_fresh_pass_duplicate/`
- Disposition-bound note: all 5 claim dispositions/summaries ≤500 chars; `close --rationale` 491 chars (under bound); close succeeded (`fully_addressed:true`, `proof_missing_claims:[]`).

## Counters
- Canonical indexed/fully-addressed/fully-read/remaining/unread/blocked/pending/watch: 4471 / 3126 / 3758 / 1345 / 713 / 414 / 214 / 4
- Read-needs-claims: 0
- All-artifact pending: 3017; noncanonical pending: 1672
- Counter audit status: **consistent** (empty mismatch list). `addressed_duplicate` status count 1124 (+1 this round).

## Division
- Cycle 31; completed rounds since follow-up 1 / 6 (rounds remaining 5).
- Review due: false.
- Round event: `division_followup_event_b8afadedd5e74dfd394f7bf49be8b190`; event_count 212; head `6459d428…`.
- Chronicle: reprojected `division_chronicle_9880214588668554c3c7ca31` (json `b9d97939…`, html `35dd8710…`); **durable inputs current (`durable_mismatches=[]`)**, only volatile `supervisor_status_sha256` mismatched (moving supervisor hash, not a durable-integrity failure).
- Note action: none due (non-return round).

## Evidence Event Store
- Validity: valid=true (verify), corrupt_lines 0.
- Pre-run baseline sequence: 896859 (`last_global_seq`, run receipt `evidence_before`).
- Current head/sequence + per-stream counts: deferred read-only debt (>10-min recompute); capture next run.
- V2 active; V1 immutable (unchanged by this evidence-only round).

## Archive
- Checkpoint due or not due: **not due for this process** — git is read-only in adapter mode; archival commits happen only in a later interactive stabilization window.
- Commit debt (exact paths this process created/edited, all UNSTAGED): `CHANGELOG.md`; `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`; the packet directory `docs/steward-notes/claude-heartbeat_1787743470_llm_marker_is_relational_verb_fresh_pass_duplicate/` (12 files). `CHANGELOG.md` and the ledger also carry prior foreign accumulated edits — separate authorship before staging.
- Verbatim introspection references if committed: n/a (no commit).
- Merge/push status and authority: none; not authorized.
