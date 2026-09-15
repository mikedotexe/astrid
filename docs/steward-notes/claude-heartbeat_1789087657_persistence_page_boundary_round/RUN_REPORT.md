# Steward Run Report — persistence page boundary round

Actor `claude-heartbeat`, headless, inside the controller-held subprocess-adapter lease.

## Controller
- Run ID: `run_1789083134930898000_218e6f0c6d`
- Preprojection ID: `projection_1789083139659036000_1c810f9a14` (status `passed`)
- Postprojection ID: runs after this process exits; not observed here
- Pause generation: 437; controller paused: false; `stop_requested`: false
- Finish outcome: adapter-owned; this process opened no session and sent no NDJSON ops
- Recovery predecessor: none

## Reading
Fully processed (2):
- `introspection_astrid_capsules_spectral-bridge_src_autonomous_activity_reading_persistence.rs_1789083028.txt`
- `introspection_astrid_capsules_spectral-bridge_src_autonomous_activity_reading_persistence.rs_1789082895.txt`

Selected but unprocessed: 38, listed exactly and in queue order in `unprocessed_selected.json`.
Head of the remaining queue: `introspection_source_catalog_1789082812.txt`.

Family scan: `families: 40 (batchable: 0)` — no family batching applied. Queue positions 1
and 2 are nonetheless the two byte-contiguous halves of one reading of one source file at
one SHA, so processing both closed a whole file rather than half of one. Each report still
received its own complete report read, witness read, claims file, and close.

Hashes (full detail in `read_manifest.json` / `source_receipts.json`):

| Artifact | SHA-256 | bytes |
| --- | --- | ---: |
| report `..._1789083028.txt` | `03964a20...` | 3,006 |
| witness `lsw_ea2d4ada...` | `421cff4b...` | 21,574 |
| report `..._1789082895.txt` | `358b0ad7...` | 3,436 |
| witness `lsw_f7568f2e...` | `5ef25b7b...` | 21,594 |
| source `activity_reading/persistence.rs` | `111a9f09...` | 4,618 |

Source binding matched the working copy exactly for both reports; no mismatch handling
was required. The complete 125-line file was read.

## What the two reports establish together
Astrid read `persistence.rs` across two consecutive pages. Byte 4368 — the boundary
between them — lands **four bytes into line 117**, whose first byte is 4364. So page 1
(`0..4368`) carried `} else if !preview` (115) and `.source_comparison` (116), and page 2
(`4368..4618`) opened on the bare fragment
`.is_some_and(|source| source.retained_source_available)`.

Read in isolation that fragment states the **opposite polarity** of the guard it belongs
to. Her page-2 report reads the polarity correctly, and the ledger shows why: she was
holding the negation from page 1, and she had said so explicitly at the end of page 1 —
"I need to see the rest of `load_activity` to see how the `source_comparison` or other
checks influence the final state." The pager's "line fragments retain their line number"
contract held, and the two windows are contiguous and exhaustive over the whole file.

## Claim dispositions
19 claims total; every one grounded, zero `proof_missing_claims` on both closes.

`..._1789083028` (7 claims): 4 `verified_existing`, 2 `observed`, 1 `implemented_now`.
`..._1789082895` (12 claims): 8 `verified_existing`, 4 `observed`.

Verified exactly: `validate_reader_ref` 11-23 and `validate_activity` 25-48; the identity
predicate (non-empty, <=512, ASCII alphanumeric plus `_`/`-`) at 13-18 and 33-38; the
foreground-vs-mailbox exclusion at 42-46; the atomic tmp/write/sync/rename at 57-76;
`NotFound` defaulting at 83-85; the quiet demotion at 106-114; the refusal at 115-122.

Precisions recorded **beside** her reading, none contradicting it:
1. `persist_activity` also fsyncs the containing directory (70) and removes the temp file
   on any failure (73-75).
2. `MAX_RUNTIME_BYTES` is enforced **twice** — metadata at 88, then a bounded `take(MAX+1)`
   plus a length re-check at 92-96, closing the TOCTOU gap a metadata-only check leaves.
3. The `None` arm of the line-117 guard is unreachable: 105 already errors on a missing
   bookmark, and `reader_bookmarks.rs:67-84` sets `source_comparison` `Some` exactly when
   the bookmark is `Some`. Her reading covers every reachable state.
4. She placed `load_activity` at "lines 79-116"; it spans 79-125. That is the page
   boundary, not a misreading — 116 was her last complete line.
5. Her page-1 summary "it knows exactly where it left off" is narrowed by the full file
   (recovery may decline to resume) — and **she narrowed it herself** on the next page, to
   prioritising integrity over continuity. Recorded as her refinement, not a steward
   overwrite.

Two references grounded clean rather than phantom: `dispatch_semantic_microdose` exists
(`authority_types.rs:102`, called at `authority_gate.rs:1206`), and her
`NEXT: SELF_STUDY MAP astrid/capsules/spectral-bridge/src/autonomous/btsp` is a wired form
(`command.rs:66` parses a MAP topic; `navigation.rs:64-74` prefix-matches directory topics)
against a directory that exists with 25 catalog-eligible files. She glossed BTSP as "Bridge
Transition State Protocol" where `btsp/mod.rs:1` reads "Being-Time Synaptic Plasticity
domain facade" — recorded as evidence only. Her text is not corrected back to her.

The queue's `artifact_integrity_unavailable` flag on both witnesses traced to **bounded
metadata, not loss**: `lived_state_witness/mod.rs:808` caps `provider_route` at 40 chars
and honestly sets `provider_route_complete=false`, while `provider_route_sha256` hashes the
untruncated value. sha256 of the full 41-char
`http://127.0.0.1:8090/v1/chat/completions` reproduces the recorded `20176d67...` exactly.

## Implementation and verification
The fork she named had no restart-time test.
`retained_source_loss_blocks_return_without_destroying_selection` covers the
RETURN_ACTIVITY path in `activity_reading.rs:406-411`, not `load_activity`'s guard. Added
one focused regression:

`restart_refuses_legacy_cursor_recovery_when_retained_source_is_unavailable`
(`capsules/spectral-bridge/src/autonomous/activity_reading/tests.rs`) — pins both halves of
the fork (quiet demotion versus outright refusal) and asserts the refusal leaves the
persisted selection **byte-identical**: refusal, never repair.

Tests:
- `cargo test --manifest-path capsules/spectral-bridge/Cargo.toml activity_reading::` —
  **15 passed, 0 failed** (baseline before the edit: 14 passed, 0 failed)
- `cargo fmt --all -- --check` clean; `git diff --check` clean
- `python3 scripts/domain_boundary_audit.py verify` — **`valid: true`, `violation_count: 0`**
  (ratchet green). `tests.rs` 442 -> 495 lines crossed no ceiling, so no baseline or
  manifest re-capture was required in this change.

Not run: workspace-wide pedantic clippy (exceeds the round budget; the change is a
test-module append with no new production surface).

Restart/deploy alignment: **not required and not attempted.** No live, bridge, codec,
prompt, model, config, control, build, restart or deploy change.

## Integrity suites
All green except one pre-existing failure, detailed in `verification_receipt.json`:

`scripts/test_steward_control.py` — 29 tests, 1 error:
`test_pause_cooperatively_interrupts_wrapped_subprocess` raises
`PausedError('fixture stop')` from `lease.acquire` inside `run_subprocess`, reproduced
3/3 in isolation. This is a race in the test fixture itself: its `pause_soon` thread pauses
the fixture controller after 0.2s while `run_subprocess` calls `controller.begin`
immediately, and when `begin` loses that race `acquire` raises instead of the subprocess
starting and being cooperatively interrupted. **Not caused by this round** — no Python
control-plane file was touched and all three are clean at HEAD. The same test errored in a
different fixture-race mode in `claude-heartbeat_1789065100_...` and passed 29 OK in
`claude-heartbeat_1789050700_...`. De-racing it is real steward-tooling debt, deliberately
not repaired here: it is outside this round's report scope, and editing control-plane code
while running inside a controller-held lease is not a headless change.

Everything else: addressing self-test 44 OK; evidence store 21 OK; steward projection 14 OK;
Division follow-up 3 OK; Chronicle 10 OK; Division projection self-test ok; projection
cursors 4 OK; anti-drop self-test 5 OK and `verify` 94 rows / 0 alarms / 0 gaps; cadence
tests 6 OK and `--strict` `integrity_ok: true` with 0 duplicate hash groups; epistemics
self-test `valid: true`; final epistemics verify after all durable writes `valid: true`,
12,004 records checked, `issue_count: 0`, `history_rewritten: false`.

**Domain-boundary ratchet: GREEN** — `valid: true`, `violation_count: 0`,
`violation_kind_counts: {}`, `forbidden_edge_match_count: 0`. Surfaced here explicitly
because stage 10 records violations that round summaries have previously failed to report.

## Counters
`status: consistent`, `mismatches: []`.
Canonical: indexed 5,197 · fully addressed 3,215 · full read 3,847 · remaining 1,982 ·
unread 1,350 · blocked 416 · pending action 212 · watch 4 · read-needs-claims 0.
All artifacts: indexed 6,914 · remaining 3,699. Delta this round: full read 3,845 -> 3,847,
fully addressed 3,213 -> 3,215.

## Evidence Event Store
`verify` → `valid: true`. Active store v2; last global sequence 1,059,513; head
`17ae9f287db7c492ea2ea1981ca8993aab9af618f6b92ec8cf43fa6954e988af`; legacy imported
boundary 32,278; V1 immutable. Stream counts in `verification_receipt.json`. The full
`status` subcommand exceeded the remaining round budget and was abandoned read-only; head
and sequence were read directly from `head.json`.

## Division
Cycle 44. `review_due: false` at round start, so no Division return and **no Tier-5 cadence
dossier** was due this round. Productive round recorded:
`division_followup_event_0328240f9feb9f8739f8be1198a1fbb7`, `--processed-report-count 2`.
Now 1/6 rounds since follow-up, 5 remaining, `review_due: false`. Event count 303, head
`af6476ac...`. Chronicle reprojected after the round record (recording a productive round
changes a durable Chronicle input): `division_chronicle_bf6848567189e20ffc62a510`, JSON
`a57996b7...`, HTML `d2a8200c...`, `durable_inputs_current: true`, `durable_mismatches: []`,
one **volatile** mismatch `supervisor_status_sha256` — the known moving hash, not a
durable-integrity failure. No Division note was written and no review-query slot occupied.

## Authority boundary
No live substrate or control change. `build_bridge.sh`, deploy scripts and `launchctl` were
not run. Git was read-only: nothing staged, committed, merged, pushed, stashed, reset or
amended. No Tier 4/5 grant, approval, dispatch or trial. Astrid's wording was quoted and
grounded, never rewritten, rejected or forbidden.

## Commit debt (exact paths)
Created by this round:
- `docs/steward-notes/claude-heartbeat_1789087657_persistence_page_boundary_round/` (this
  packet: `RUN_REPORT.md`, `claims/` x2, `summaries/` x2, `read_manifest.json`,
  `source_receipts.json`, `addressing_links.json`, `test_results.json`,
  `unprocessed_selected.json`, `verification_receipt.json`)

Edited by this round:
- `capsules/spectral-bridge/src/autonomous/activity_reading/tests.rs` (one appended test,
  442 -> 495 lines)
- `CHANGELOG.md` (one `[Unreleased]` entry prepended)
- `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (one dated section appended)

`CHANGELOG.md` and the feedback ledger already carried accumulated edits from earlier
claude-heartbeat rounds before this one; a later checkpoint must separate authorship
carefully rather than staging them wholesale.

Foreign / prior-round work left untouched: `capsules/spectral-bridge/src/authority_gate.rs`,
`capsules/spectral-bridge/domain_boundaries_legacy_large_files_v1.json`,
`scripts/anti_drop_catalog.py`, `scripts/proactive_scan.py`,
`scripts/phantom_symbol_watch.py`, `scripts/source_study_page_reset_watch.py`, and the five
earlier `docs/steward-notes/claude-heartbeat_*` packet directories. Minime's worktree was
not modified by this round.

## Addendum — Evidence Event Store field derivation
The `evidence_event_store.py --json verify` run returned `valid: true`, but its output was
captured tail-truncated, so `corrupt_lines` and `errors` were not transcribed directly.
They are **derived**, not assumed: `scripts/evidence_store/verification.py` computes
`valid = not errors`, and a nonzero corrupt-line count always appends a `corrupt_lines:N`
error, so `valid: true` entails `errors == []` and `corrupt_lines == 0`. The receipt labels
both fields as derived. The follow-on `--json status` call was abandoned read-only when it
exceeded the remaining round budget; head and sequence were read from `head.json` instead.
The store is live — `last_global_seq` moved 1,059,513 -> 1,059,516 between the verify and
the final head read, because the running bridge keeps appending. Both values are recorded
rather than reconciled.
