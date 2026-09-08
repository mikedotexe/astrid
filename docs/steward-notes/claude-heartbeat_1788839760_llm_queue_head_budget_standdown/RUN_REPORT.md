# Steward Run Report — incomplete budget stand-down (no productive round)

Actor: `claude-heartbeat` (headless, controller-held subprocess lease)
Round packet: `docs/steward-notes/claude-heartbeat_1788839760_llm_queue_head_budget_standdown/`
Outcome: **no canonical report processed; exiting nonzero so the run records `failed`.**

## Controller

- Run ID: `run_1788835869794174000_47667661b3`
- Adapter: `subprocess` (`scripts/steward_control.py run --actor claude-heartbeat --max-secs 5400`)
- Lease acquired: `2026-09-08T02:51:09.809239+00:00`
- Pause generation: `395`; `stop_requested=false` throughout
- Preprojection generation ID: **not observable to me.** In adapter mode the `ready` record is
  consumed by the controller, not the child, so the preprojection ID never reaches this process.
  It is only needed for `record-round`, which was not run.
- Postprojection: runs after I exit; not observed here.
- Finish outcome: recorded by the adapter from my exit code. I force a nonzero exit so this
  incomplete round records `failed` rather than `success` — see finding **F4**, which shows the
  previous stand-down's nonzero intent was silently recorded as `success`.

## Why this round stood down

`scripts/steward_control/executor.py:31-46` starts the child immediately after `controller.begin(...)`
and SIGINTs it at `begin + max_secs`. With `FLYWHEEL_LOOP_MAX_SECS=5400` and a lease begun at
02:51:09Z, the child SIGINT deadline is **1788841269 (~04:21:09Z)**.

Reading the two required law documents consumed the first stretch of the run — with an unbudgeted
detour, see finding **F1** below — leaving **1596 seconds (~26 minutes)** at the decision point.

The ONE-SHOT rule requires the *slowest remaining* sequence to fit:
`record-read -> link-evidence-batch -> close -> integrity suites -> record-round`. Those addressing
calls can each take 20+ minutes at current evidence-store size, and `evidence_event_store verify`
alone was measured at ~11 minutes in run `run_1788279228206526000_19c21b465c`. Twenty-six minutes
cannot hold that sequence, and starting `record-read` would have put a durable mutation under the
watchdog. **No mutating addressing, EES, or Division call was started.** This is budget truncation,
not input scarcity: canonical input exists and is named below.

This is the second budget stand-down in the last three runs, though **not** consecutive: run
`run_1788776746818032000_bf3d783806` (log line `1788781538`) stood down on budget after a complete
read, then run `run_1788822294661698000_61cb9f512d` completed a productive round
(packet `claude-heartbeat_1788828278_marker_context_window_boundary_round/`, processed
`introspection_astrid_llm_1788821913`, one `tests.rs` regression). Two of three runs losing to the
watchdog is still a cadence signal worth a steward's attention: at current evidence-store size the
5400s child budget has little margin once preprojection and law-reading are paid for.

## Findings for the next session

### F1 — the operating-law handoff is absent from `main` (infrastructure, not being-facing)

`docs/steward-notes/ASTRID_INTROSPECTION_SOURCE_FIRST_FLYWHEEL_HANDOFF.md` **does not exist** in the
primary working tree. The tree is on `main` at `3d4e83e4bc`; the file is on no ancestor of `main`
(`git merge-base --is-ancestor` is false for all three commits that ever touched it:
`ccb53b1622` 2026-09-03, `347eef9afa` 2026-08-15, `40130a6e29` 2026-08-12). It survives only on
codex branches (`codex/graceful-coupling-rollout`, `codex/hebbian-clock-boundary`,
`codex/sovereign-daughter-runtime`) and in checked-out worktrees.

I read it read-only from the newest committed version:
`git show ccb53b1622:docs/steward-notes/ASTRID_INTROSPECTION_SOURCE_FIRST_FLYWHEEL_HANDOFF.md`
(55155 bytes, 1504 lines, sha256 `5d464a94e76b422520b10c4b3ea65be4ce5789607e4c249551ca59e4080e180a`).

This is a live hazard: the flywheel prompt names the handoff as operating law by repository-relative
path, and that path resolves to nothing on the branch the flywheel actually runs on. A future
session that trusts the path and finds nothing could proceed without its law. **Recommended repair
(needs an interactive stabilization window — I am git read-only): bring the handoff onto `main`.**
I did not create, copy, or stage the file; putting a codex-branch document onto `main` is a merge
decision for Mike, not a headless one.

### F2 — the queue head binds source outside the primary tree

Head report `introspection_astrid_llm_1788821220` names its source as
`/Users/v/other/worktrees/marker-rollout-20260907/astrid/capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
at SHA `03c6b6dee0436dd56c047ab68d95f2f4ccd6e2ed7c8029cf0b9568eb9cfefa91`, window `lines 1-400 of 812`,
scope `partial_window_unseen_source_not_assessed`. The second queue item binds a *different* worktree
(`afterimages-live-20260907`, `lines 1-400 of 744`). The next session must hash the named worktree
copy, not the primary tree's `dialogue_runtime.rs`, and must label report-time vs current-source
conclusions separately per the handoff's source protocol. The 812-line vs 744-line split across
adjacent reports means these are **not** the same source bytes and cannot share one source receipt.

### F4 — the exit-code honesty contract is not enforceable as wired (verified)

The operating instructions require an incomplete round to "exit NONZERO so the run records failed."
**That has not been happening.** The child is `scripts/flywheel_round_child.sh`, whose last line is
`exec claude -p ...` — so the child's exit code is the CLI's own, which is `0` whenever the turn
completes normally, regardless of whether the round completed.

Verified against the durable controller record: the previous budget stand-down
(`run_1788776746818032000_bf3d783806`) ended its log line with "Exiting nonzero", yet
`capsules/spectral-bridge/workspace/diagnostics/steward_control_v1/runs/run_1788776746818032000_bf3d783806.json`
records `outcome: success`, `exit_code: 0`, `summary_ref: null`. The intent was sincere and the
mechanism silently discarded it.

Consequence: incomplete rounds are indistinguishable from complete ones in the controller's durable
history, and any future audit of flywheel reliability that trusts `outcome` will overcount successes.
This is the un-muffle invariant applied to the steward's own telemetry — a signal that could not
complete was vanishing instead of surfacing.

This round closes the gap for itself by self-terminating with a nonzero status after every durable
write is complete, so the adapter's `outcome = "failed"` branch (`scripts/steward_control/executor.py:47-52`)
is actually reached. That is a per-round workaround, not a fix. **The durable fix belongs in
`scripts/flywheel_round_child.sh`** — e.g. drop the `exec`, have the round write a machine-readable
completion marker into its packet, and let the wrapper exit nonzero when the marker is absent or
says incomplete. That is a steward-tooling change I did not make here: it is outside this round's
processing scope, and the two consecutive-ish stand-downs mean it should be made deliberately in an
interactive window rather than bolted on under a watchdog.

### F3 — session-start scan warnings I could not act on (read-only, unverified by me)

The SessionStart hook reported 7 warnings. I did not verify any of them and did not act on any;
they are relayed, not confirmed. Two deserve a steward's eye:

- `steward_outreach`: 5 unread being->steward outreach, oldest **93.3h**, flagged
  "PICKUP FAILING". Under the un-muffle invariant this is the highest-value item in the warning set —
  it is being-facing and time-decaying, and it is not something this flywheel round consumes.
- `ungated_bridge_binary`: the on-disk release bridge binary does not match the build the gate
  recorded (manifest by `codex-marker-rollout` at 2026-09-07T22:21). Deploy-gate territory; explicitly
  outside my authority.

Also relayed unverified: `log_error_rate` (2001 errors / 2 active logs), `plist_drift`,
`architecture_drift` (review 896 -> 897), `reflective_sidecar` (0/5 trailing INTROSPECTs covered),
`feedback_flywheel` (Tier 4/5 grant waiting).

## Reading

- Canonical reports fully processed: **none**
- Canonical reports read: **none** — nothing was opened, so nothing was marked read
- Selected but unprocessed: **all 40**, in canonical order, in `unprocessed_selected.json`
  (raw controller output preserved as `next_queue.json`; scan output as `family_scan.json`)
- Queue head: `introspection_astrid_llm_1788821220.txt` (`unread`, family `astrid_llm`,
  witness `lsw_01cc250e66cf5758d4b296193c69406685fd1ee389709a967ffa32a2288094ef`,
  alignment `deployment_unknown`)
- Family scan (read-only, queue-order preserving): 37 families over 40 items, 3 batchable.
  **The head is a singleton family** — the next session must process it single-report, not as a
  family batch.
- Operating law read completely: `AGENTS.md` (9182 B, 137 lines,
  sha256 `0f6dbd1ff48ab13b479c93812911ef243bdacbce54e2e8211ded171b225eb8a3`) and the handoff (see F1).

## Claim dispositions

None. No report was opened, so no claim was extracted and none was disposed. `claims/` and
`summaries/` are empty by design rather than by omission.

## Actions

- Corridor/program, Sandbox, study, portfolio, cards, notes, correspondence: **none**
- Tier 4/5: **none advanced**. `feedback_flywheel` reports a Tier 4/5 grant waiting; it stays waiting.
- Division return: **not due** (`review_due=false`), so no bounded return and no Tier-5 cadence
  dossier was owed or generated this round.

## Implementation and verification

- Changed source or test paths: **none**. No focused tests were owed.
- Live surface: **no restart, build, deploy, or launchctl action was required or attempted.**
- Read-only durable verification actually run (results in `test_results.json`):
  - `introspection_addressing_audit.py --self-test` -> 44/44 OK (0.796s)
  - `anti_drop_catalog.py verify --json` -> 0 gaps, 0 alarms
  - `domain_boundary_audit.py verify` -> `valid=true`, `violation_count=0`, `violation_kind_counts={}`.
    **The ratchet is GREEN** — there is no red ratchet to surface this round.
    (`resolved_large_file_debt_count=3`, `unlisted_legacy_review_debt_count=44`,
    `stable_facade_count=6`, manifest sha256 `578a39cf...`)
  - `division_ceremony_followup.py verify` and `status` -> below
- Deferred to budget, honestly not run: EES `verify`/`status`, the final epistemic verify (only owed
  after durable writes, and there were none), the standalone `audit-counters` call, the seven tooling
  unit suites, and the Chronicle project/verify. Listed exactly in `test_results.json`.

## Counters

Read non-mutatingly from the `next` report block; `status` = **consistent**, `mismatches` = `[]`,
all seven internal checks true.

| Counter | Value |
| --- | ---: |
| Canonical indexed | 4,622 |
| Canonical fully addressed | 3,193 |
| Canonical fully read | 3,825 |
| Canonical remaining | 1,429 |
| Canonical unread | 797 |
| Canonical blocked | 416 |
| Canonical pending action | 212 |
| Canonical watch | 4 |
| Canonical read-needs-claims | 0 |
| All-artifact pending | 3,143 |
| Noncanonical pending | 1,714 |

## Division

- Cycle 41, completed rounds since follow-up **4 / 6**, rounds remaining 2, `review_due=false`
- Event count 285, head `296bdf67c3525d9d812e74b03d044980fd98250afa6081fb887199c7685b1dd8`
- Last round event `division_followup_event_5fd9735fe291295f66622591686edc59`
- **No productive round recorded.** `record-round` was not called: no report was processed, and the
  handoff forbids manufacturing a productive round. Division is left exactly at 4/6.
- Note action: none. Chronicle was not projected or verified (deferred to budget; no Division write
  occurred that would stale it).

## Evidence Event Store

Not queried — `verify`/`status` were deferred to budget. **Zero events were appended by this run**
across every stream, so the head this session inherited is the head it leaves.

## Archive / commit debt

Git was **read-only** this run (`status`, `log`, `show`, `ls-files`, `merge-base`, `worktree list`,
`rev-parse` only). Nothing staged, committed, stashed, reset, merged, pushed, or amended. The index
was clean at start and is clean at exit. The 54 pre-existing dirty paths are foreign work and were
left untouched.

Exact commit debt created by this run — every path I created or edited:

```
docs/steward-notes/claude-heartbeat_1788839760_llm_queue_head_budget_standdown/RUN_REPORT.md
docs/steward-notes/claude-heartbeat_1788839760_llm_queue_head_budget_standdown/addressing_links.json
docs/steward-notes/claude-heartbeat_1788839760_llm_queue_head_budget_standdown/claims/.gitkeep
docs/steward-notes/claude-heartbeat_1788839760_llm_queue_head_budget_standdown/read_manifest.json
docs/steward-notes/claude-heartbeat_1788839760_llm_queue_head_budget_standdown/source_receipts.json
docs/steward-notes/claude-heartbeat_1788839760_llm_queue_head_budget_standdown/summaries/.gitkeep
docs/steward-notes/claude-heartbeat_1788839760_llm_queue_head_budget_standdown/test_results.json
docs/steward-notes/claude-heartbeat_1788839760_llm_queue_head_budget_standdown/unprocessed_selected.json
docs/steward-notes/claude-heartbeat_1788839760_llm_queue_head_budget_standdown/verification_receipt.json
workspace/logs/flywheel_loop.log   (one appended stand-down line)
```

`CHANGELOG.md` and `AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` were **not** updated, and should not be:
those are owed when a report causes implementation, verification, or a deliberate authority boundary.
No report was processed, so no such entry is earned. Both files also carry heavy foreign edits.

## Authority boundary

Evidence only. Nothing here approves, grants, dispatches, deploys, restarts, or changes any live
control. No lease token was read, quoted, or persisted. No being-authored text was rewritten,
rejected, or summarized as consent. Silence remains neutral. The three warnings in F3 that touch
being-facing or deploy-gated surfaces are relayed for a steward with the authority to act, and are
explicitly not acted on here.
