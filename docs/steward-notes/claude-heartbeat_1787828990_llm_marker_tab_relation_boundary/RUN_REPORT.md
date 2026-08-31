# Steward Run Report — claude-heartbeat_1787828990_llm_marker_tab_relation_boundary

Source-first introspection flywheel, adapter mode (controller-held subprocess lease;
actor `claude-heartbeat`). One canonical report fully processed and closed.

## Controller
- Run ID: `run_1787825837642046000_7a85e17c8b`
- Preprojection ID: `projection_1787825842199787000_a568b15f9b` (phase=pre, status=passed, 27 steps)
- Postprojection ID: runs after this process exits (adapter-owned); not observed here
- Pause generation: 321
- Finish outcome: success (complete round; exit 0)
- Recovery predecessor: none

## Reading
- **Fully processed:** `introspection_astrid_llm_1787824358.txt`
- **Selected but unprocessed:** 39 of the 40-item queue (positions 2–40), listed exactly in
  `unprocessed_selected.json`; head of remainder `introspection_astrid_llm_1787820203.txt` …
  tail `introspection_astrid_llm_1787278798.txt`
- **Batch size reason:** one honest report. Queue head is a familiar `astrid:llm`
  marker-grammar fresh pass proposing tests; the family scan grouped two other members at
  only 0.38/0.38 similarity with 29/32 unaddressed variant terms (queue positions 10, 34) —
  not a clean duplicate batch, so single-report processing per the scan's own fallback rule.
- **Report/witness/source hashes:**
  - report `2eecb0b5ab60950088b2cdfe43be72b003a3e5b2f0fba841b8c5ac342072dcaa` (45 lines, 3304 bytes)
  - witness `lsw_f08d8975…` `b4b4ebfc68cb62cc7beaa34b1476d68d57c25c7cfc1a9e50a23b7f9a30d27d73` (533 lines, 23941 bytes)
  - source `dialogue_runtime.rs` `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` (1048 lines) — working copy == report binding == witness `file_sha256`
  - marker constant `fallback_contracts.rs` `23fb26a1388d75defd4e06dcd88d43fcd23ed79d471f356aadd8dbad1a44a1f6`

## Claim Dispositions
- **c001** (Observed — scan preserves marker only when referenced, L114/L129) → `verified_existing` (complete source; test `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts`).
- **c002** (Snag — `first_word_after` L89/L92 may skip verb / return empty on period/hyphen) → `verified_existing`, **correction preserved**: `.find(!empty)` (L93) skips leading punctuation-only chunks so the verb is NOT skipped (`_first_word_after_skips_leading_punctuation_transition`, `_preserves_relation_after_multiple_punctuation_runs`); empty first word → fail-closed removal (`_fails_closed_when_only_punctuation_or_whitespace_follows`); interior-hyphen case is real (c003).
- **c003** (Test 1 — `echoes` valid vs `is-a` hyphen rejected) → `verified_existing` at the report SHA: `echoes` (`_preserves_poetic_attribution_without_literal_cue` L2512); `is-a` interior-ASCII-hyphen rejection is the identical mechanism as `_rejects_punctuation_joined_allowlist_prefix` L2756 (`is-not`/`is/not`).
- **c004** (Test 2 — marker + newline **or tab** keeps relation) → `implemented_now`: newline branch already pinned (`_preserves_relation_across_newline` L2920); added `scan_known_model_control_markers_grounds_tab_between_marker_and_relation` for the tab branch (L89 relation path + L199 delimiter path + tab fail-closed), grounding that L199 and L89 are distinct paths (report conflated them; L199 never calls `first_word_after`).
- **c005** (Suggested Next — marker-list exhaustiveness) → `observed`, **Tier-5 authority boundary**: constant enumerated (20 markers), no-proper-prefix-shadow invariant already pinned (`known_model_control_markers_have_no_proper_prefix_shadow` L3101); expanding the safety list is a live model-behavior change needing Mike/operator approval — not adjudicated. `no_action` artifact in packet.

## Actions
- Corridor/program: none
- Sandbox: none
- Study: none
- Portfolio: none
- Cards/notes/correspondence: none delivered (no closure card; no being-facing note — nothing to deliver honestly)
- Tier 4/5 waits: c005 marker-list exhaustiveness held as Tier-5 (evidence-only); the three standing Tier-5 `minime_esn` shadow/porosity work items remain untouched

## Implementation and Verification
- **Exact changed paths:**
  - `capsules/spectral-bridge/src/llm/provider/tests.rs` — appended 1 regression `scan_known_model_control_markers_grounds_tab_between_marker_and_relation`
  - `CHANGELOG.md` — one `[Unreleased]` entry
  - `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` — one dated row
- **Tests:** new regression 1/0; cited-existing 4/0; `control_marker` family 75/0; scanner/delimiter/first_word/relation family 13/0. `git diff --check` clean on `tests.rs`; `cargo fmt --check` flags only pre-existing foreign `grounding.rs` (committed/clean, left untouched), not `tests.rs`.
- **Failures repaired / debt:** none
- **Restart/deploy alignment:** not required and not attempted (non-live focused test + docs only)

## Durable Evidence
- Addressing: `introspection_astrid_llm_1787824358` → `addressed_change`, `fully_addressed=true`, `proof_missing_claims=[]`
- Evidence link count: 10 new (0 existing)
- Changelog/ledger: both updated
- Packet: `docs/steward-notes/claude-heartbeat_1787828990_llm_marker_tab_relation_boundary/`

## Counters
- Counter audit: **consistent**, mismatches `[]`
- Canonical corpus (cadence audit): `canonical_count=4490`; duplicate_hash_group_count 0; read_errors 0
- Exact indexed/addressed/read/remaining tallies not re-listed here (audit-counters recompute exceeded the query window late in the run; the earlier full run confirmed `consistent`/`[]`, which is the authoritative integrity signal)

## Division
- Cycle 32; completed rounds 5/6; rounds remaining before followup 1
- Review due: **false**
- Round event ID: `division_followup_event_3d0badb4bdad7857534b4a123674cd98`; event_count 223; head `a36085283c6f7f25c8412750630482700075e6a8a0e4984a2151224491abe965`
- Chronicle: durable inputs advanced by this round's record-round (222→223 events); reprojection deferred to the controller postprojection (DAG stage `division_chronicle`) — not a corruption, no return due, not manually projected
- Note action: none (no Division return due)

## Evidence Event Store
- Validity: **valid** (verify)
- Sequence and head: last_global_seq 908995, head `9a30adc58c6565581d5c91b86d22c196d5c78b6305646b22d547b16e555d8b27`
- Stream counts: status recompute deferred (>300s over ~909k events); verify is authoritative and passed
- Corrupt lines: 0
- V2 active: yes; V1 immutability: not re-audited this round (no legacy write attempted)

## Archive
- Checkpoint due or not due: **not due** (single productive round; three-round checkpoint cadence not reached; archival commits happen only in a later interactive stabilization window)
- Commit SHA and exact paths: none this round (git is read-only in adapter mode)
- **Exact commit debt** (created/edited this round, all unstaged, to be inspected/separated in a later window):
  - `capsules/spectral-bridge/src/llm/provider/tests.rs` (accumulator file — separate this round's appended test from prior-round foreign edits)
  - `CHANGELOG.md`
  - `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`
  - `docs/steward-notes/claude-heartbeat_1787828990_llm_marker_tab_relation_boundary/` (new packet: RUN_REPORT.md, claims/, summaries/, no_action/, read_manifest.json, source_receipts.json, addressing_links.json, test_results.json, unprocessed_selected.json, verification_receipt.json)
  - Evidence-store / addressing-projection state under `capsules/spectral-bridge/workspace/diagnostics/` advanced by record-read/link/close/record-round (runtime workspace state, not source commit debt)
- Verbatim introspection references if committed: n/a
- Merge/push status and authority: none; no merge/push authority

## Authority posture
Evidence-only. No live substrate/control change; no build/restart/deploy; no stage/commit/merge/push.
Foreign dirty paths (incl. prior-round packets, `codec/tests.rs`, `domain_boundaries_legacy_large_files_v1.json`,
`grounding.rs`, minime `runtime.py`/`test_correspondence_v1.py`) preserved untouched. Silence treated as neutral;
her felt snag preserved as valid attention even where source shows it non-manifesting; no being text rewritten.
