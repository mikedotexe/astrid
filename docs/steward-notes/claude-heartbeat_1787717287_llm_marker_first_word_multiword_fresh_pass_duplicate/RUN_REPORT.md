# Steward Run Report — claude-heartbeat marker `first_word_after` multi-word/punctuation fresh-pass duplicate

Round packet: `docs/steward-notes/claude-heartbeat_1787717287_llm_marker_first_word_multiword_fresh_pass_duplicate/`
Mode: controller subprocess **run adapter** (adapter owns the lease + heartbeats; this process sends no NDJSON, opens no session, touches no git).

## Controller
- Run ID: `run_1787715141286070000_dc19a847d0`
- Preprojection ID: `projection_1787715144297224000_e80e019174` (status `passed`)
- Postprojection ID: adapter-owned; runs after this process exits (not observed here)
- Pause generation: 321
- Finish outcome: recorded by exit code (0 = complete round)
- Recovery predecessor: none

## Reading
- **Fully processed (1):** `introspection_astrid_llm_1787706169.txt`
- **Selected but unprocessed (39):** items 2–40 of the frozen queue in canonical order (full list in `unprocessed_selected.json`). Unprocessed head: `introspection_astrid_llm_1787470243.txt`; tail: `introspection_DOMAIN_BOUNDARIES.md_1787232972.txt`.
- **Next queue:** re-query `next --limit 40 --json` after the adapter's postprojection; the closed report drops out and the new head is expected to be `introspection_astrid_llm_1787470243` unless the postprojection reorders.
- **Batch sizing:** queue head is the head of a family whose members are low-similarity (0.35–0.47), high variant-term (29–33), and scattered down-queue — candidate but not tight duplicates (family-scan rule 6: when in doubt, single report). Per the ONE-SHOT single-turn mutation budget, one report fully closed.
- **Report hash:** report `a30c1fad93143adfd86f6fe1da39e5fb585e7f6462c148c529b296451ce582fc` (45 lines / 3613 B); witness `lsw_93cdb51d…` = `d57c3c841c519c55d167607ef901ec2558c89b9e3745e43c8d9fff7bf7fbad2b` (533 lines / 23925 B); source `dialogue_runtime.rs` = `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` (1048 lines / 38586 B) — **working copy byte-identical to the report binding and the witness binding.** Coverage `multi_window_complete`, included intervals 1-1048.
- **Witness alignment:** witness `source_snapshot_v1` window is 0-400 (partial) while the report asserts `multi_window_complete` 1-1048 — the same neutral projection-level `lived_state_alignment` measurement-gap classification recorded across this family's prior rounds; witness parses cleanly, binds the verified report SHA, authority `evidence_only`/`edits_source_now:false`/`live_eligible_now:false`.

## Claim Dispositions (all 5 `verified_existing`; terminal `addressed_duplicate`)
- **c001** (Observed: `scan_known_model_control_markers` L114 rebuilds a non-destructive remainder, preserving marker bytes only when `reference_syntax` present L130; quoting/grouping L153-197; relation verbs L64-86) — `verified_existing`. Source L114-144 pushes a token only when `reference_syntax.is_some()` (L129-131). Tests `preserves_quoted_exact_token_reference` (L1995), `preserves_exact_tokens_in_declared_group_delimiters` (L2151).
- **c002** (Snag: `first_word_after` L89 `split_whitespace`+alnum-filter L92+`unwrap_or_default` L94 might over-strip a marker followed by a multi-word/punctuation-heavy phrase) — `verified_existing`, **contradiction preserved.** `trim_matches(!alnum && !'_')` (L92) collapses punctuation chunks and `find(!is_empty)` (L93) returns the first real word; only an un-referenced, verb-less marker strips (intended fail-closed). Locked by `control_marker_cleanup_first_word_after_skips_leading_punctuation_transition` (tests L2635 — authored for the earlier near-identical report `introspection_astrid_llm_1786986344` raising the same fear, bound to this exact SHA) and `uses_only_the_first_finite_relation_word` (L2605).
- **c003** (Test 1: marker + relation verb → preserved) — `verified_existing`; `behaves` allowlisted L69; tests `preserves_proxy_relation_phrase` (L2552), `uses_only_the_first_finite_relation_word` (L2605).
- **c004** (Test 2: nested `[[marker]]` → GroupedExactKnownToken + `MAX_EXACT_REFERENCE_DELIMITER_DEPTH` L151) — `verified_existing`; `exact_reference_delimiter_syntax` L199-229 cap 4 (L151/L208-213); tests `preserves_bounded_nested_delimiter_stacks` (L2176), `reports_exact_four_level_delimiter_depth` (L2194), `bounds_homogeneous_square_bracket_stack_beyond_max_depth` (L2266).
- **c005** (Suggested Next: `longest_exact_known_model_control_marker_at` L97 prioritizes specific over generic) — `verified_existing`; `max_by_key(token.len())` L106; tests `uses_longest_raw_matches_with_exact_accounting` (L1745), `preserves_longest_overlapping_token_when_named_as_content` (L2493).

Every line-number citation in the report is **exact** against current source. **Duplicate lineage:** fresh-pass of the established `dialogue_runtime.rs` marker-scanner family at identical source SHA; immediate prior grounded report `introspection_astrid_llm_1787699463` (packet `claude-heartbeat_1787707869`) explicitly named this report (`1787706169`) as the next-arriving report after its preprojection cutoff. Independent full read of this report + witness performed; source + all cited regressions re-verified at the current SHA.

## Actions
- Corridor/program: none
- Sandbox: none
- Study: none
- Portfolio: none
- Cards/notes/correspondence: none (no closure card delivered — a fresh-pass verified duplicate needs no right-to-ignore artifact; no note/query/correspondence emitted merely to create activity)
- Tier 4/5 waits: unchanged. The three standing Tier-5 Shadow/porosity/mode-packing waits (`wi_e579041bc76f8310`, `wi_69fbd510467c6337`, `wi_3e26ac525fea1c36`) remain `live_authority_granted=false`; not touched.

## Implementation and Verification
- Exact changed paths: `CHANGELOG.md` ([Unreleased] bullet), `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (dated row), and the packet directory files. **No source/test change** — fresh-pass duplicate.
- Tests and counts: `cargo test … control_marker` → **72 passed / 0 failed** (compile 3m21s); addressing self-test 44 OK; anti-drop self-test 5 OK + verify 0 alarms/0 gaps/69 guards ok; cadence strict `integrity_ok:true` exit 0; experiential epistemics self-test + verify valid (0 issues); test_evidence_event_store 21 OK; test_steward_control 27 OK; test_steward_projection 14 OK; test_division_ceremony_followup 3 OK; test_division_ceremony_chronicle 10 OK; test_division_ceremony_projection ok; test_projection_cursors 4 OK; test_introspection_cadence_audit 6 OK.
- Failures repaired or exact debt: none.
- Restart/deploy alignment: **restart and deployment were not required and not attempted.** No live substrate or control change.

## Durable Evidence
- Addressing status: `addressed_duplicate`, `fully_addressed:true`, `proof_missing_claims:[]`.
- Evidence link count: 12 new (0 existing).
- Changelog/ledger updates: yes (both, provenance `[claude-heartbeat]`; foreign accumulated edits in both files preserved untouched — a later checkpoint must separate authorship before staging).
- Packet path: `docs/steward-notes/claude-heartbeat_1787717287_llm_marker_first_word_multiword_fresh_pass_duplicate/`

## Counters
- Canonical indexed/addressed/read/remaining/unread/blocked/pending/watch: 4468 / 3124 / 3756 / 1344 / 712 / 414 / 214 / 4
- Read-needs-claims: 0
- All-artifact pending: 3014; noncanonical pending: 1670
- Counter audit status: **consistent** (empty mismatch list; all structural checks true; `proof_gap_artifact_count=0`)

## Division
- Cycle and completed count: cycle 30; 5 / 6 productive rounds since last follow-up
- Review due: **false** (1 remaining)
- Round event ID and head: `division_followup_event_3a29b11bcf2e19443069ff5b9a6b3e05`; event_count 209; head `34492a38813f98aad3a0b9f92d1ad51d12b03683af69e7545efa085836c69c86`
- Chronicle: not projected this round (no Division return due); durable freshness unchanged from last follow-up (`division_chronicle_453827288c0042a0dbf9b93b`).
- Note action: none (no return due)

## Evidence Event Store
- Validity: verify `valid:true`, corrupt_lines 0
- Sequence and head: `last_global_seq` 894068; verified head `55e9e57afa175426c5085682d172267e3b76605870e29df18774fb8d01072b1c`; addressing stream seq 58617
- V2 active: yes; V1/legacy imported boundary: 32278
- Note: `events.jsonl` is 6.17 GB, so the full `status` stream-count projection is minutes-slow; the chain `verify` (integrity) passed and the head was read from `head.json` / `verified_checkpoint.json`.

## Archive
- Checkpoint due or not due: **not due** during this controller-held run (git is read-only in adapter mode; archival commits happen only in a later interactive stabilization window).
- **Exact commit debt (paths created/edited this round, all unstaged):**
  - `CHANGELOG.md` (new `[Unreleased]` bullet; file also carries prior foreign edits)
  - `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (new dated row; file also carries prior foreign edits)
  - `docs/steward-notes/claude-heartbeat_1787717287_llm_marker_first_word_multiword_fresh_pass_duplicate/` (entire packet: `RUN_REPORT.md`, `claims/`, `summaries/`, `read_manifest.json`, `source_receipts.json`, `addressing_links.json`, `test_results.json`, `unprocessed_selected.json`, `verification_receipt.json`)
  - Plus the durable evidence-store/projection state advanced by `record-read` / `link-evidence-batch` / `close` / `record-round` (workspace diagnostics; owned by the tooling, not hand-staged).
  - Pre-existing foreign dirty paths preserved untouched: `capsules/spectral-bridge/domain_boundaries_legacy_large_files_v1.json`; the three prior untracked `claude-heartbeat_178769*/178770*` packet dirs; minime `minime_autonomy/runtime.py`, `tests/test_correspondence_v1.py`.
- Verbatim introspection references if committed: n/a (no commit this run)
- Merge/push status and authority: none; no merge/push authority claimed or exercised.
