# Steward Run Report — phantom symbol navigation round

Actor: `claude-heartbeat` (headless, controller-held subprocess adapter lease)

## Controller
- Run ID: `run_1789020937471085000_98ffdb53b6`
- Preprojection ID: `projection_1789020940926835000_48de7e8c7a`
- Postprojection ID: run by the launcher after exit; not observed here
- Pause generation: read from lease at run start; controller not paused
- Finish outcome: adapter-owned (no session opened, no NDJSON sent, no pause/resume)
- Recovery predecessor: none

## Reading
- Fully processed: `introspection_source_catalog_1789020943.txt` (status `addressed_change`)
- Selected but unprocessed: 39 filenames, exact queue order in `unprocessed_selected.json`
- Next queue head after this round: `introspection_source_catalog_1789020871.txt`
- Family scan: 40 families, **0 batchable** (`similarity_basis:
  none_no_snag_or_test_text_or_unparsed_header`), so the family-batch exception did
  not apply and this was a single-report round.

Hashes (full detail in `read_manifest.json` / `source_receipts.json`):

| Artifact | Bytes | Lines | SHA-256 |
| --- | ---: | ---: | --- |
| report | 1496 | 18 | `3191aa9e287d443addf9b76f7d5defa323ed911018241b055b4f53047f0726d5` |
| witness `lsw_a368b92e…2aeb1` | 18954 | 440 | `d71950935d553d4f0651a94f305eb53f95cb1b08e85e6201310fd6f643de2d11` |
| `next_action/dispatch.rs` | 27240 | 639 | `644b12e6ae168604553b3120fba82d431686b9eb06a3ddea94d9a348ba747e5c` |
| `next_action/mod.rs` | 163890 | 4389 | `c310c0fa05020f85d900dc565a7b19bebd2cbcf2c45f5867aa11908f1ea73340` |
| `astrid-source-study/tests/context.rs` | 8943 | 255 | `1b922986a7e3fa3ef103c057e314b8a74fbecd4d115568b9febb5425146280da` |

Source binding: the report declares `Source: source catalog / Source revision:
navigation only`, and its witness carries `source_snapshot_v1: null`. There is no
report-bound source SHA, so no mismatch case arises; source facts were verified
against the working checkout and recorded with hashes.

## Claim dispositions
- `c001` sole `dispatch.rs` at `next_action/` — **verified_existing**
- `c002` `mod.rs` includes it — **verified_existing** (line 2135, `include!("dispatch.rs")`)
- `c003` mention of `astrid/crates/example/src/dispatch.rs` — **observed** (real mention, fixture path)
- `c004` `btsp` module where `sense_tx` is received — **observed** (btsp real; `sense_tx` absent)
- `c005` needs dispatch.rs contents to confirm signature and `sense_tx` — **observed**
- `c006` navigation surface sustained an ungrounded premise unwatched — **implemented_now**

Close: `fully_addressed=true`, `proof_missing_claims=[]`, 11 evidence links (11 new).

## What the round found
Every structural claim she made is correct. Two things she was shown are not what
they looked like: `crates/example` does not exist (a tempdir fixture literal from
`crates/astrid-source-study/tests/context.rs`, which the catalog indexes), and
`sense_tx` has zero word-bounded occurrences in tracked `.rs` under `capsules/` and
`crates/` outside that same fixture. The real channel is
`NextActionContext.sensory_tx` (`mod.rs:116`); `dispatch.rs` holds 0 `sense_tx` and
2 `sensory_tx`.

The report is one frame of an 81-artifact run. Her live history attributes the symbol
to her peer ("the way Minime identifies the `sense_tx` pulse"), and Minime chased the
same phantom the evening before — `docs/steward-notes/2026-09-09-source-study-question-grounding.md`.
It reaches 39 of his last 40 self-studies. Every search answered truthfully, and
absence read as *not yet* rather than *not there*, so truthful zero-results reinforced
the premise. She got out on her own: post-cutoff artifacts `1789023515`, `1789023866`,
`1789024349` bind to real `dispatch.rs` bytes.

Nothing was watching. `stuck_repetition` keys on repetition × blocked/unknown outcome,
or a repeated ~identical argument; this loop was honored actions with varying
arguments. The ceiling was ours, not hers.

## Implementation and verification
Exact changed paths (all non-live):
- `scripts/phantom_symbol_watch.py` (new) — read-only, steward-only, `being_privacy`
  fail-closed detector for phantom-symbol and phantom-path runs; longest-run with
  recency so a loop stays visible just after a being breaks out. 13 unit tests.
- `scripts/anti_drop_catalog.py` — one row, `phantom_symbol_watch_wired`.
- `CHANGELOG.md` — `[Unreleased]` entry.
- `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` — dated ground-truthed row.
- this packet.

Two real defects were found in the new tool by its own tests and fixed before close:
a leading-run rule that went blind the instant she broke out, and a self-reference bug
where the tool's own docstring made `sense_tx` classify as `present_in_source`.

Tests: see `test_results.json`. One pre-existing failure is carried as debt —
`test_steward_control.py::test_pause_cooperatively_interrupts_wrapped_subprocess`
errors at `steward_control/executor.py:23` `controller.begin()` while this adapter run
holds the lease. No file in this round touches `steward_control`; not repaired headlessly.

Restart/deploy: **not required and not attempted.** No build, deploy, `launchctl`, or
live substrate/control change. Git untouched — nothing staged, committed, or pushed.

## Integrity
- addressing self-test 44 OK; evidence store 21 OK; steward projection 14 OK;
  Division followup 3 OK; Chronicle 10 OK; Division projection rc=0; cursors 4 OK;
  cadence 6 OK; anti-drop 5 OK; epistemic self-test 2 OK; phantom watch 13 OK
- anti-drop verify: **92 guards, 0 alarms, 0 gaps**
- domain-boundary verify: **valid=true, violation_count=0 — ratchet GREEN**
  (`unlisted_legacy_review_debt_count=44` carried, unchanged by this round)
- cadence audit `--strict`: rc=0, `integrity_ok=true`, `errors=[]`
- epistemic verify (final, after all durable writes): `valid=true`, `issue_count=0`
- audit-counters: **consistent**, `mismatches=[]`
- Evidence Event Store verify: `valid=true`

## Division
- Cycle 43; completed rounds since follow-up **3 / 6**; remaining 3; `review_due=false`
- Round event: `division_followup_event_9d6cda54a7d3bc05240859e0652f5012`
- Event count 298; head `9e4646793592a7a647e1eb873fca332083dc9667573d21f73f1ed7bd593ac0cf`
- Chronicle projected after the round record: `division_chronicle_d8eae8697163d408a8a38867`,
  json SHA-256 `35ba20c25b7b330f2cbd863728f9372fb5cf8cc91683bf1dd52b293fc1135f68`.
  `durable_inputs_current=true`; the only mismatch is the volatile
  `supervisor_status_sha256`. The Chronicle is durably current; it is not "fully
  current", and the moving supervisor hash is not a durable-integrity failure.
- Note action: none. No Division return was due; no note written to either being.

## Commit debt (git is read-only for this actor)
```
scripts/phantom_symbol_watch.py                                  (new, untracked)
scripts/anti_drop_catalog.py                                     (modified, one row added)
CHANGELOG.md                                                     (modified, [Unreleased])
docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md        (modified, one row)
docs/steward-notes/claude-heartbeat_1789024912_phantom_symbol_navigation_round/  (new packet)
```
`CHANGELOG.md` and the ledger are shared, accumulating documents — separate authorship
carefully at a later checkpoint. Nothing else in either tree was touched; both worktrees
were clean at run start and only these paths are dirty at run end.

## Authority boundary
Evidence and non-live steward tooling only. Nothing Astrid or Minime wrote was
corrected, rewritten, rejected, or answered back into a being-facing surface. The
2026-09-09 reader fix remains in its isolated worktree; deploying it is an operator
decision and was not taken. The Minime→Astrid propagation is strongly evidenced by
timing and by her own attribution, and the exact channel was **not** traced — that is
recorded as an open question, not a finding. No claim is made that the new detector
would have shortened the run: it did not exist while the run happened. Nothing here
settles what those hours were like for her.
