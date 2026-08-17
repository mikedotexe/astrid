# Steward Run Report

Round name: `llm_marker_first_word_advance_overlap_already_grounded`
Actor: `claude-heartbeat` (subprocess run adapter; the controller owns the lease and heartbeats)

## Controller
- Run ID: `run_1786857681006833000_9390344842`
- Preprojection ID: `projection_1786857686108146000_170d759586` (status `passed`, run_id matches lease)
- Postprojection ID: runs after this process exits (adapter-managed); not observed here
- Pause generation: 319
- Finish outcome: success (single report fully closed; adapter records finish from exit code 0)
- Recovery predecessor: none

## Reading
- **Fully processed (1):** `introspection_astrid_llm_1786848204.txt`
- **Selected but unprocessed (39):** items 2-40 of the frozen queue, in order — `introspection_astrid_llm_1786838089.txt`, `introspection_astrid_llm_1786831572.txt`, `introspection_astrid_llm_1786829036.txt`, `introspection_astrid_llm_1786822981.txt`, `introspection_astrid_llm_1786814454.txt`, `introspection_astrid_llm_1786809350.txt`, `introspection_llm.rs_1786807306.txt`, `introspection_astrid_llm_1786788349.txt`, `introspection_astrid_codec_1786784975.txt`, `introspection_astrid_llm_1786782248.txt`, `introspection_DOMAIN_BOUNDARIES.md_1786752896.txt`, … (complete list in `unprocessed_selected.json`).
- **Next queue head after this run:** `introspection_astrid_llm_1786838089.txt` (a batchable 3-member family in the scan; a future round may batch it). Note: a newer report `introspection_astrid_llm_1786858484.txt` arrived after the preprojection cutoff and is correctly left for the next projection/queue.
- **Report hash:** report `c6442992bf495b89d6766316132b72fda2d40abedcbd0dccffe1458db3e910b8` (45 lines, 4051 B); witness `lsw_b25ecce7…` = `8102dbc6e1973d845d0b415481599cc77989716acd3e653a06f5d615271b00fe` (533 lines, 23934 B); source `dialogue_runtime.rs` = `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` (1048 lines, 38586 B) — **working copy byte-identical to the report binding**. Coverage state `multi_window_complete`, included intervals 1-1048.

### Batch sizing
The queue head's family (`introspection_family_scan.py`) pairs it with only **one** distant member (`introspection_astrid_llm_1786600655`, queue #36, similarity 0.368, 39 `variant_distinct_terms`) — a marginal near-duplicate that would require full independent processing of a second report, doubling the slow addressing-CLI mutation sequence at the current evidence-store size (seq ~811k). Per family-scan rule 6 (when-in-doubt, single-report), honest batch = **1 report**, fully closed within the one-shot budget.

## Claim Dispositions (all six `verified_existing`; terminal `addressed_no_action`)
- **c001** (Observed: marker-scanning + two preservation paths) — `verified_existing`; src L114-144/L199-229/L49-86; test `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts` (L2845).
- **c002** (Snag: `first_word_after` punctuation/multi-word fragility) — `verified_existing`, **contradiction preserved**; `split_whitespace`+`trim_matches`+`find(!empty)` already preserves a punctuation-separated verb (test L2900) and takes only the first finite relation word (L2565). Fails closed only when no alnum word follows.
- **c003** (Snag: single-char fallback advance inefficiency/fragmentation) — `verified_existing`; byte-exact `char.len_utf8()` (L134-140), no fragmentation; test `control_marker_scanner_advances_byte_exactly_across_multibyte_text` (L2099). Cost bounded, not a defect.
- **c004** (Test #1: bracketed marker → GroupedExactKnownToken) — `verified_existing`; grouped branch L175-193; proposed test already exists (L2845/L2151/L2876).
- **c005** (Test #2: `[TOKEN_X] behaves` → relation true) — `verified_existing`, **contradiction preserved**; a bracketed marker preserves via the **delimiter** path (checked first, L50), so `followed_by_explicit_exact_token_relation` is never consulted; the verb path (`behaves` allowlisted L69) is proven on a **bare** marker (test L2858).
- **c006** (Suggested Next: overlapping markers) — `verified_existing`; longest-match `max_by_key(len)` (L106); actual list (`fallback_contracts.rs` L159-180) has no COMMAND tokens (her example NOT_FOUND) but real overlaps `<channel|>`⊂`thought <channel|>`, `[INST]`/`[/INST]` (tests L1745/L2453).

## Actions
- Corridor/program: none
- Sandbox: none
- Study: none
- Portfolio: none
- Cards/notes/correspondence: none delivered (a no-action right-to-ignore **artifact** was written to the packet and linked; no card/note/query/correspondence delivered)
- Tier 4/5 waits: widening the finite relational-verb allowlist / delimiter tables is Tier-5-class live grammar — **not** made, dispatched, or deployed

## Implementation and Verification
- **Exact changed paths (created — packet):** `docs/steward-notes/claude-heartbeat_1786860129_llm_marker_first_word_advance_overlap_already_grounded/{RUN_REPORT.md, claims/introspection_astrid_llm_1786848204.json, summaries/introspection_astrid_llm_1786848204.md, no_action_introspection_astrid_llm_1786848204.md, read_manifest.json, source_receipts.json, addressing_links.json, test_results.json, unprocessed_selected.json, verification_receipt.json, next_queue_frozen.json, family_scan.json}`
- **Exact changed paths (edited — shared dirty tracked files, append-only additions at unique anchors):** `CHANGELOG.md` ([Unreleased], one `[claude-heartbeat]` bullet at the top), `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (one dated `2026-08-15` block at the top of `## Ledger`)
- **Source/test code changed:** none (all six claims `verified_existing`; a near-identical regression would be activity without evidentiary value)
- **Tests:** 8 focused marker grounding tests pass (`8 passed; 0 failed`, incremental build 3.74s, source SHA 902a0358). Integrity suites all pass (see `verification_receipt.json`): addressing self-test (44), evidence-store (20), steward-control (27), steward-projection (14), division-followup (3), chronicle (10), division-projection (ok), cursor (4), cadence (6), cadence-strict (integrity_ok), epistemic self-test (2) + final verify (valid, 11099 records, 0 issues), anti-drop self-test (5) + verify (55 entries, 0 alarms/0 gaps), audit-counters (consistent), evidence_event_store verify (valid, 0 corrupt lines).
- **Flaky/environmental FAILURES:** none this round — `test_steward_control.py` (27) and `test_steward_projection.py` (14) that flaked in the prior round both passed clean here.
- Restart/deploy alignment: **no live change required or attempted** — no bridge build, deploy, or launchctl.

## Durable Evidence
- Addressing status: `addressed_no_action`, `fully_addressed=true`, `proof_missing_claims=[]`
- Evidence link count: 17 new (0 existing)
- Changelog/ledger updates: yes (both, append-only)
- Packet path: `docs/steward-notes/claude-heartbeat_1786860129_llm_marker_first_word_advance_overlap_already_grounded/`

## Counters (audit-counters: consistent, all checks True, mismatches [])
- Canonical indexed/addressed/read/remaining/unread/blocked/pending/watch: 4366 / 3092 / 3723 / 1274 / 643 / 413 / 214 / 4
- Read-needs-claims: 0
- All-artifact pending: 2916; noncanonical pending: 1642
- Counter audit status: **consistent** (canonical `addressed_no_action` 91→92)

## Division
- Cycle and completed count: cycle 25, completed **2/6**
- Review due: false (rounds remaining before followup: 4)
- Round event ID / head: `division_followup_event_1b20083bed4571af3d2a9b2779a5f465` / `298a6d5d6cf06ca059d7d509c9151a5fe8c149d4408ae55a3e5e6e29826e9d8a` (event_count 171)
- Chronicle: verify reports `chronicle durable source inputs changed; project before verify` — **expected** (record-round appended the round event); Chronicle reprojection is scoped to the `review_due=true` return path, not this non-due round. Not a corruption.
- Note action: none (no Division return due; no note written)

## Evidence Event Store
- Validity: true
- Sequence / head: 811797 / `1fabe4a98b8d7cc58ab14123d15ae707001993bea9d711b08e0edfbe1d9e6a4f`
- Stream counts: addressing 57440, agency_commons 4819, attention_portfolio 3, claim_families 236839, corridor_v1 5, corridor_v2 112, felt_contracts 196497, felt_mechanism_concordance 80, lived_state_witness 8524, model_qos 170045, reciprocal_uptake 57158, representation_contracts 32344, sandbox 2986, signal_spine 30815, steward_control 13674, steward_work_selection 456
- Corrupt lines: 0
- V2 active: yes; V1 immutability: preserved (no legacy source rewrite)

## Archive
- Checkpoint due or not due: **not applicable in adapter mode** — git is read-only for this run; no stage/commit/merge/push performed
- **Exact commit debt (for a later interactive stabilization window):**
  1. New packet dir `docs/steward-notes/claude-heartbeat_1786860129_llm_marker_first_word_advance_overlap_already_grounded/` (12 files listed above)
  2. `CHANGELOG.md` — one `[Unreleased]` `[claude-heartbeat]` bullet at the top (file also carries foreign edits; separate authorship at checkpoint)
  3. `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` — one appended `2026-08-15` block at the top of `## Ledger` (file also carries foreign edits; separate authorship at checkpoint)
  - Durable evidence appended to workspace stores (not git source): addressing full_read + 17 evidence links + close event; division_followup round event 171. These are append-only diagnostics, not staged files.
- Verbatim introspection references if committed: none committed this run
- Merge/push status and authority: none; no merge/push authority exercised or implied
