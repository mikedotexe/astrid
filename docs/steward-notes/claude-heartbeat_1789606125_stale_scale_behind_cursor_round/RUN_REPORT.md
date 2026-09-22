# Steward Run Report — claude-heartbeat, stale_scale behind the cursor

## Controller
- Run ID: `run_1789601431310501000_07a09d6e59` (subprocess adapter, lease held by the controller)
- Preprojection ID: `projection_1789601435127975000_f9d8c79f5e` (phase `pre`, status `passed`)
- Postprojection ID: runs after this process exits; not observable from inside the round
- Pause generation: 447
- Finish outcome: adapter-owned; this round completed its work and wrote the completion receipt
- Recovery predecessor: none
- Adapter-mode boundaries honoured: no steward session opened, no NDJSON ops, no pause/resume, no
  lease token read or persisted, git read-only throughout

## Reading
- Fully processed: `introspection_minime_minime_src_sensory_bus.rs_1789601340.txt`
- Selected but unprocessed: 39 filenames, listed in queue order in `unprocessed_selected.json`
- Queue head after this round: `introspection_minime_minime_src_sensory_bus.rs_1789601004.txt`
  (next forward window, bytes 102876..107073)
- Family scan: 40 families, **0 batchable** (each queue item is a distinct forward window of the
  same sequential walk; `similarity_basis: none_no_snag_or_test_text_or_unparsed_header`), so this
  was a single-report round by the scan's own verdict, not by preference
- Hashes: report `ef2a74508c8dcfa9c834737c0c01c3b57afe394360a951651b72ce90573fcddb` (43 lines,
  3708 B); witness `lsw_f28ccee4…` sha `336bf20e81a3633f413861ebefdc1a662f8d7226f614db04e87cb59796228c35`
  (498 lines, 21323 B); source `minime/src/sensory_bus.rs` sha
  `3fc6bd2a16bd78c5caa496f2a6dccbc67928da4fbded123998f59a82bcd4aa3a` (4404 lines, 168439 B) —
  **identical to the report binding, working copy clean**, so no report-time reconstruction needed

## Claim Dispositions
Ten claims, all evidenced (`fully_addressed: true`, `proof_missing_claims: []`):

| Claim | Classification | Short disposition |
| --- | --- | --- |
| c001 lane layout | verified_existing | 2930-2949, exact |
| c002 `Z_DIM` "likely 64 or larger" | verified_existing | 66 (line 25); her own page's comment 2888-2890 says so |
| c003 stale decay of semantic weight | verified_existing | 2919-2929 |
| c004 three sovereignty knobs | verified_existing | two act at 2941-2943; `memory_decay_rate` acts at 1720-1721 on the window |
| c005 `effective_semantic` formula | verified_existing | line 2943 verbatim |
| c006 global noise ±(level×0.05) | verified_existing | confirmed by line 2966, past her window |
| c007 bus as freshness-weighted integrator | observed | 2891-2950 + SampleMeta 2971-2987 |
| c008 "need to see how `stale_scale` is calculated" + `CONTINUE` | implemented_now | helper at 1377-1417, all call sites behind her; new reach test |
| c009 standing question: pressure/entropy/decay | verified_existing | `semantic_stale_ms` 1710-1729 with multipliers at 171 and 251 |
| c010 per-piece delivery reach | observed | two pieces delivered and behind her; two never opened in this walk |

## Actions
- Corridor/program: none
- Sandbox: none
- Study: none
- Portfolio: none
- Cards/notes/correspondence: none emitted, none delivered
- Tier 4/5 waits: none created; none discharged

## Implementation and Verification
- Added `crates/astrid-source-study/tests/behind_cursor_helper_midwalk_reach.rs` (274 lines,
  sha `61ad7d9bc0dd0baacd83e8db889d85bd342ec20d6c1f2bcd17048c2a1f389384`) — 4 read-only reach pins
  for the mid-file behind-cursor shape
- Tests: `cargo test -p astrid-source-study --test behind_cursor_helper_midwalk_reach` 4 passed;
  `cargo test -p astrid-source-study` **185 passed / 0 failed**; `cargo fmt -p astrid-source-study
  -- --check` clean; `cargo clippy -p astrid-source-study --tests` clean (two pedantic warnings in
  the new file fixed before the recorded run)
- Failures repaired or debt: none outstanding. `scripts/test_steward_control.py` failed one
  load-sensitive fixture (`test_pause_cooperatively_interrupts_wrapped_subprocess`) while ten suites
  ran concurrently; re-run serially it passes. No code changed, and the same flake is on record from
  an earlier round.
- Restart/deploy alignment: **not required and not attempted.** No live surface was touched.

## Durable Evidence
- Addressing: `record-read` → `link-evidence-batch` (16 new links, 0 pre-existing) → `close`
  (`addressed_change`); materialized status confirms `fully_addressed: true`,
  `proof_missing_claims: []`, 10/10 claims with evidence
- Changelog: `CHANGELOG.md` `[Unreleased]` — new steward section
- Ledger: `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` — dated ground-truthed row
- Packet: `docs/steward-notes/claude-heartbeat_1789606125_stale_scale_behind_cursor_round/`

## Counters
- Canonical indexed 7306 / fully addressed 3261 / fully read 3893 / remaining 4045 / unread 3413 /
  blocked 416 / pending action 212 / watch 4
- Canonical read-needs-claims: 0
- All-artifact indexed 9023, remaining 5762
- Counter audit: **consistent**, `mismatches: []`

## Division
- Cycle 50, completed rounds 4/6, rounds remaining 2
- Review due: **false** both before and after this round — no Division return was owed, therefore
  no Tier-5 cadence dossier was owed or generated
- Round event: `division_followup_event_696dd7b263a547d9672d96c535f2b00f`, event count 348,
  head `9dcae364d788fb7a0882744f08a761216970b40ee4364f3b5b7f3710934fec08`
- Chronicle: **expected-stale**, not current. `verify` reports "chronicle durable source inputs
  changed; project before verify" — the ordinary state after a round-append; the postprojection's
  `division_chronicle` stage resolves it. Reported as a volatile-input staleness, not a durable
  integrity failure.
- Note action: none. No Division note was due and none was written.

## Evidence Event Store
- Validity: true; corrupt lines 0; errors []
- Sequence 1116602, head `b04a74a24ca0928502e8833c3026639d6fd7c30d03e3e7ffb4a722d59c2c374f`
  (run began at seq 1116210, head `fbd98d17…`)
- V2 active; V1 legacy sources untouched
- Stream-by-stream enumeration not expanded this round (budget); `status` ran clean

## Integrity Suites
addressing self-test 44 OK · evidence store unit 21 OK · controller 29 (1 load-sensitive flake,
passes serially) · projection 14 OK · division followup 3 OK · division chronicle 10 OK · division
projection self-test ok · cursors 4 OK · cadence unit 6 OK · cadence `--strict` `integrity_ok:true`
0 errors · anti-drop self-test 5 OK · anti-drop verify **100 rows, 0 gaps, 0 alarms** ·
domain-boundary verify **valid, 0 violations — ratchet GREEN** · epistemics self-test valid ·
final epistemics verify **12425 records, 0 issues, no history rewrite** · audit-counters
**consistent** · EES verify **valid, 0 corrupt**

## Archive — exact commit debt
Nothing was staged or committed; the index is clean and `git diff --check` passes. This round's
paths, for a later interactive stabilization window:

**Created**
- `crates/astrid-source-study/tests/behind_cursor_helper_midwalk_reach.rs`
- `docs/steward-notes/claude-heartbeat_1789606125_stale_scale_behind_cursor_round/` (9 files:
  `RUN_REPORT.md`, `addressing_links.json`, `claims/introspection_minime_minime_src_sensory_bus.rs_1789601340.json`,
  `family_scan.json`, `read_manifest.json`, `source_receipts.json`,
  `summaries/introspection_minime_minime_src_sensory_bus.rs_1789601340.md`, `test_results.json`,
  `unprocessed_selected.json`, `verification_receipt.json`)

**Edited (both already carried accumulated edits from earlier rounds — separate authorship
carefully before staging)**
- `CHANGELOG.md`
- `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`

**Foreign, untouched:** `capsules/spectral-bridge/src/action_continuity/tests.rs`,
`capsules/spectral-bridge/src/autonomous/inquiry/parsing.rs`,
`crates/astrid-kernel/src/kernel_router.rs`, `crates/astrid-kernel/src/maintenance.rs`,
`crates/astrid-source-study/tests/component_map_sibling_reach.rs`,
`crates/astrid-source-study/tests/unrooted_map_topic_reach.rs`, and the seven earlier
`docs/steward-notes/claude-heartbeat_*` packets. Minime's worktree is clean and was read only.

## What this round does not establish
The reach pins are fixture-based and read-only. They do not change what she is offered, do not
assert what she recalls, and do not decide whether the delivery surface should ever name which
reach shape a reader is in — that remains an open design question for Mike.
