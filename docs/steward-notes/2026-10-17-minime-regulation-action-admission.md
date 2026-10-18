# Minime: Fresh Regulation Requests and Truthful Idle Status

## Status and Scope

Source-bound immutable candidate qualified and gracefully activated on October
18 and integrated into both local main branches. No push is included.
Mike approved
repairing the disconnected request path through ordinary admission and exposing
the specific idle reason. This is not an instruction to replay old requests or
force writing. The separately approved, coordinated Minime-agent-only activation
and its preservation evidence are recorded in the October 18 section below.

Paired isolated worktrees, both on `codex/minime-regulation-actions-20261017`:

- `/Users/v/other/worktrees/minime-regulation-actions-20261017/minime`, based on
  local main `b8d47c7ad7adbf21b17f19942e6d3489b4ed65fb`.
- `/Users/v/other/worktrees/minime-regulation-actions-20261017/astrid`, based on
  local main `297023ab75ebdc1ee0beb28848d8b50fb4965191`; documentation and
  launch-reconciliation packaging only, with no bridge runtime change.

Canonical Minime was clean. Canonical Astrid's model-upgrade changes are foreign
and remain untouched. Controller pause generation 528 belongs to
`codex-mlx-upgrade`, with no active lease or projection at inspection. No
automation was resumed and no controller session was acquired for this isolated
interactive repair. During October 17 qualification, no live shared index or
service wrapper was mutated.

## Evidence and Diagnosis

Times below are October 17, 2026, America/Los_Angeles (PDT).

1. The last sampled public journal was
   `workspace/journal/!journal_2026-10-17T13-53-31.510090.txt` in Minime, SHA-256
   `c0a8c485a27eeee05baf30572b6b5b661a0211cb8c36a41889773116b37aa98a`.
   This filename/hash is a publication boundary, not a claim about its meaning.
2. Metadata-only inspection found subsequent private-writing generations. The
   last job, `job_minime_1792273439990_write-continue`, completed at
   `2026-10-17T21:49:17.297011Z` (14:49:17 PDT), with no active worker in the
   inspected status. Verified receipt `writing-1451` reported ordinary provider
   stopping and no selected NEXT. No private prose, title or draft reference
   content was used in fixtures or copied into this note.
3. The agent remained alive as PID 60407, boot
   `7f1fb8d6a04f40aab474e21bbe54013e`. The installed runtime SHA-256 was
   `72048dcb6d9a545f314f35db9f6f5376fb78fe45912dd10443f055e1b1c6e804`, matching
   the loaded source in the prior inspection. This did not demonstrate an MLX
   crash or active-generation stall.
4. `logs/autonomous-agent.log` contains `Being requested action: introspect`
   at 15:16:20.423, 16:36:37.911, 16:42:18.652, 16:48:05.489 and 17:34:04.860.
   Line numbers at inspection: 1354306, 1354494, 1354509, 1354523 and 1354631.
   `_self_regulate` stored recognized optional JSON actions in
   `_sovereignty_requested_action`; source inspection found no consumer.
5. Without NEXT, the existing fallback predicates need not select anything.
   The bounded prior telemetry inspection covered 4,500 samples across about
   10,641 seconds, with eigenvalue 17.07834-19.13663 and delta -0.20375-0.59326.
   The synthetic regression uses eigenvalue 18.67 and delta 0.024 and reproduces
   the no-route result. No fallback threshold or cadence is changed here.

Public silence therefore contains distinct intervals: selected private writing,
then no pending choice, then regulation requests that never entered the action
queue. Missing NEXT is not explicit REST and does not establish a wish for
indefinite silence. Conversely, fixing the queue does not establish an obligation
to keep writing, an experiential explanation, or subjective improvement.

## Implementation

### Fresh Requests Through the Existing Queue

`minime_autonomy/action_scheduling.py` recognizes only the six already advertised
names: `self_experiment`, `compose_audio`, `reservoir_read`,
`reservoir_resonance`, `self_study` and `introspect`. Their corresponding existing
commands enter `_persist_pending_next_action(..., only_if_unqueued=True)` under
the existing cross-process state lock; no separate scheduler or retry store is
introduced.

An explicit NEXT from the same response wins, including REST. An existing
in-memory or durable choice wins. Protected work, shutdown/recovery and an
unadmitted choice defer the request without retaining it for later replay.
Queue failure preserves the existing fail-closed handoff behavior. Execution
still passes through ordinary `_decide_action` and `_execute_action`, including
charter, live-control, evidence, research and executor checks. Queue acceptance
is explicitly not execution or completion.

The existing choice envelope records the regulation-response hash and origin
`regulation_review_json_action`, with `authored_next_line=false`. It does not
pretend the JSON field was an authored NEXT footer. Existing graceful seal and
restore preserve the command, selection identity and envelope. There is no new
authored-state schema or migration, and no historical request/log replay.

The regulation menu now describes bare INTROSPECT as open reflection, matching
the actual shared-reader route, rather than calling it targeted source study.
No change to regulation JSON controls, gains, thresholds, timing or fallback
selection is included.

### Content-Free Status

The existing source-status publication adds `action_scheduler_v1`. It contains
a pending-choice boolean, a timestamped last observation and the last bounded
regulation-request disposition. It contains no private command payload, title,
prose, model reply or new private path. It is a process-local observation, not a
durable history or an inferred account of intentions.

Specific observations distinguish no NEXT/no threshold action, explicit REST,
protected-work wait, cooldown with a pending choice, missing telemetry,
primary-job activity, guard refusals, dispatch and errors. A primary job can be
queued, running or awaiting cancellation; `primary_job_active` deliberately does
not claim that token generation has started. Existing job records remain the
source for delivery and completion. REST retains its existing one-decision
semantics rather than becoming a new persistent hold.

Both touched pending-choice loop logs now use the existing private-action
redactor. New operational logic lives in the small support module; the already
large runtime receives only routing/status hooks and the queue-envelope option.
No unrelated monolith refactoring is bundled with this repair.

## Qualification

Only synthetic requests, sources and stubbed providers are used. Pytest's
live-write/network protection stays enabled. The unchanged, already qualified
shared helper is reused without a Rust build:

`/Users/v/other/worktrees/truthful-return-20261016/bridge-stage-01/helpers/astrid-source-study`

SHA-256: `8ce052182613a1616e4301a6b2e9a6a290edb6b3d8db2b2b178001d7d5311085`.

Environment: `PYTHONDONTWRITEBYTECODE=1` and `ASTRID_SOURCE_STUDY_BIN` set to that
helper; commands use `python3 -B -m pytest`. No release identity, debugger profile
or build-cache setting is changed. No cache cleanup or substantial build occurs.

- Focused queue/review/status/handoff/study suites: 248 passed before the final
  queued-job status clarification.
- Complete Minime Python suite before the final clarification: 2,299 passed,
  one skipped, 167 subtests passed. Final-source rerun: **2,300 passed, one
  skipped, 167 subtests passed** in 115.62 seconds.
- Astrid fixture-only launch reconciliation, restart, finish-accepted-work,
  NEXT handoff, launch-inventory, paired-handoff, controller and Evidence Event
  Store checks: **127 passed, 40 subtests passed** in 17.52 seconds.
- Both worktrees pass `git diff --check`. No Rust source changed, so no Rust
  rebuild, Clippy run or altered cache is represented as necessary qualification.
- The real adapter regression reaches verified reflection delivery with an
  unrelated retained experiment, preserves the empty source bookmark/notebook,
  publishes one synthetic introspection and retains its explicitly selected
  next action.
- Coverage includes all six recognized names; absent/malformed/unknown requests;
  same-response NEXT and REST; protection/stop/recovery; a newer durable queue;
  queue write failure and corruption; ordinary guard refusal; handoff identity;
  and actual scheduler cycles for idle, cooldown, job activity, dispatch, missing
  telemetry and exceptions.

Unsuccessful attempts are retained here rather than reported as product bugs:
the first focused run had 71 passes and two incorrect assertions (bare
INTROSPECT uses the `self_study` dispatcher to deliver reflection, and the legacy
SELF_EXPERIMENT command is not categorically refused without an experiment).
The next run had 233 passes and seven fixture assertions incorrectly forbidding
an unrelated normal sovereignty save. Corrections assert the real contract,
including an injected live-control refusal, without weakening runtime gates.
The fake regulation-provider fixture now explicitly stubs the control socket;
the test network guard prevented live access on the earlier attempt.

Final-source test commands:

```sh
env PYTHONDONTWRITEBYTECODE=1 \
  ASTRID_SOURCE_STUDY_BIN=/Users/v/other/worktrees/truthful-return-20261016/bridge-stage-01/helpers/astrid-source-study \
  python3 -B -m pytest -q tests

env PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=scripts python3 -B -m pytest -q \
  scripts/test_reconcile_minime_launch.py scripts/test_restart_minime_agent.py \
  scripts/test_finish_minime_work.py scripts/test_minime_next_handoff.py \
  scripts/test_launch_inventory_contract.py scripts/test_paired_minime_handoff.py \
  scripts/test_steward_control.py scripts/test_evidence_event_store.py
```

The first command runs in the isolated Minime worktree, the second in the
isolated Astrid worktree. Final SHA-256 identities:

| Minime-relative path | SHA-256 |
| --- | --- |
| `minime_autonomy/runtime.py` | `22ba4371081d989a74bbaaab7aade3f221086fdd068d99b5e20e1353aa19974a` |
| `minime_autonomy/action_scheduling.py` | `e91469b5a1f412f79b8945d736b33ca349539f30b1e59098a4ffde1564873a43` |
| `tests/test_regulation_action_scheduling.py` | `cd499de775847c5f8d9c1e599585867b759bbdadad84741b517ee84034b1beb4` |

Owned candidate paths are precisely these three Minime files and its
`CHANGELOG.md`, plus Astrid's `CHANGELOG.md`, this note and
`docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`. New files remain
untracked; no index operation has occurred.

At final read-only inspection, source status checked at 18:00:22 PDT still
reports PID 60407, the same boot identity, no loaded-source drift, idle lifecycle,
zero active/running jobs and the same last completed writing job. `ps` confirms
the process remains alive. This is confirmation of non-activation, not a claim
that live writing has recovered.

## Immutable Agent Qualification

Completed October 17, after Mike requested the next qualification step. All
artifact paths in this section are relative to
`/Users/v/other/worktrees/minime-regulation-actions-20261017`.

The explicit reconciliation allowlist now includes
`minime_autonomy/action_scheduling.py`. New regressions require it, verify its
exact snapshot bytes alongside the modified runtime, and prove canonical inputs
are untouched. Initial reconciliation/inventory checks: 19 passed.

`minime-reconciliation-01/source` freezes **101 launch inputs** and the unchanged
registry seed asset, with read-only files/directories and content hashes. The
running source-status inventory and installed launch plist match canonical
Minime's 100-input source. Only `minime_autonomy/runtime.py` is modified and
`minime_autonomy/action_scheduling.py` is added. The complete overlay preserves
the installed regulation, continuation, writing limits and shared-helper logic.
This is an offline source candidate, not a relocatable installation, a launchd
selector or an independently frozen third-party dependency environment.

Qualification outcomes:

- `preservation-qualification-01/qualification.json`: **171 checks** passed
  through the current same-schema writing-admission qualifier. Both old/new
  stage arguments deliberately identify the same currently installed immutable
  bridge/helper. The synthetic reader, writer, private-input admission, operator
  inspection, pending-delivery and checkpoint checks preserve the unchanged
  helper contract; this does not request a bridge deployment or schema migration.
- `frozen-agent-qualification-01/tests.xml`: **262 tests passed** in 38.11
  seconds against the frozen snapshot, not the mutable candidate checkout.
  Before/after module `__spec__.origin`, implementation code filenames and hashes
  bind 54 initially loaded and 57 finally loaded autonomy modules to that
  snapshot. The complete source-level 2,300-test qualification above still
  applies to the unchanged runtime identities; it is not relabelled as a full
  frozen-snapshot run.
- The frozen probe also launches separate Python processes against old and new
  source snapshots. Old-to-new startup retains an exact synthetic WRITE choice,
  selection identity and envelope. New-to-old startup retains the regulation
  INTROSPECT request and its new provenance envelope. Both directions preserve
  pending-state bytes exactly, append normal restore acknowledgement, request no
  generation and open no live authored state. This is preservation evidence,
  not authority to downgrade running code or restore old backups.
- `release-compatibility-01.xml`: **129 tests and 40 subtests passed** in 17.06
  seconds, covering launch reconciliation/inventory, sanctioned restart and
  finish-accepted-work wrappers, native/paired NEXT handoff, controller and
  Evidence Event Store behavior.

The reproducible one-off driver is `qualify-frozen-agent.py`. Its source-bound
receipt also records all test-source hashes, interpreter identity, unchanged
canonical and selected inventories, unchanged helper identity, import origins
and four cross-version handoff records. Tests use synthetic state, stubbed
providers and the existing live-write/network guard; the handoff subprocesses
add a blanket socket-connect refusal. No model generation or service signal is
used. Both qualifiers retain `activation_performed=false` and
`live_eligible_now=false`; larger-capacity qualification is excluded.

Retained unsuccessful attempt: `helper-preservation-01/failure.json` reports
`old executable creates genuine pre-v5 checkpoint`. The historical geometry
migration qualifier expected a genuinely older pre-v5 reader, whereas this
release changes no reader/schema. It was not modified or treated as a pass.
The current same-schema preservation qualifier above was used instead.

| Artifact | SHA-256 |
| --- | --- |
| `minime-reconciliation-01/reconciliation.json` | `69f0e6b87e6eaed60f1b2d4d6cb4b0a7a1a34691853ca35d159c63f83efd9b3d` |
| `preservation-qualification-01/qualification.json` | `082582e66ca1d67aeaae26552a04cc35a33772e0f113ee1fc2d18cac19022f3a` |
| `frozen-agent-qualification-01/qualification.json` | `dbc6a8b5005f44e31a6cc4604df7949ff20cf2e1784d9ced37b05b8f27d6fcd6` |
| `frozen-agent-qualification-01/tests.xml` | `2f92667770da02d584009aeeeba0b61c8e11092e3997981548df02badf9baa06` |
| `release-compatibility-01.xml` | `12268796ac775025429513520fba9288f807dedf279c5da87b72e321504cddca` |
| `qualify-frozen-agent.py` | `9a9fff28e6efca4965e1090be63ae41fc95d7e6a1f01b5f2fb71a670a2de41c1` |
| `helper-preservation-01/failure.json` | `1d149d330c1b435d0e24e8cfe70be1f8e5a28ab641a4dd2fa01cc0bd591f1711` |

Additional owned repository paths for this step are Astrid's
`scripts/reconcile_minime_launch.py` and
`scripts/test_reconcile_minime_launch.py`. No runtime implementation changed
after the prior source qualification. No substantial build, cache deletion,
dependency installation, live checkpoint copy or private-content fixture was
needed. Keep the external artifact directories and their original receipts.

## Pre-Activation Boundary, October 17

The candidate changes Minime's runtime and adds one helper module. Astrid source,
shared-reader schemas, private writing, engine, provider, model, sensory and
visual services are unchanged. Capacity qualification remains separate.

Immutable source freezing and synthetic old/new NEXT preservation are now
qualified. Before activation: obtain fresh Minime-agent-only rollout approval
and a coordinated service window; revalidate the current launch inventory,
installed plist, source status, helper selection and protected processes against
these exact receipts; install only the two reviewed runtime paths through the
sanctioned finish-accepted-work/handoff procedure. Stop on source drift, foreign
service work, failed drain, failed readiness or incompatible state. Do not force
termination or restore older state over newer authored work.

The latest read-only app status for `Assess Astrid MLX update`
(`01a14133-d657-7cc1-b49d-3d87b04c7aa3`) reports active review of an interruption
rehearsal journal. Its host closeout is reported clean, but its production check
remains held pending acceptance review. That is progress, not completion or a
shared deployment window. We did not message or interrupt that task. Controller
pause 528 remains owned by `codex-mlx-upgrade`, with no active lease/projection
and valid indexed-tail evidence/V1 immutability at read-only inspection.

After approved activation: verify fresh process/boot identities, all loaded
source hashes, exact NEXT handoff and unchanged protected services. Observe
fresh requests and their admission/delivery records without replaying the five
old requests or prompting confirmation of improvement. Coordinate explicit-path
Git integration separately; this note grants no activation or index authority.

## Approved Agent-Only Activation, October 18

Mike explicitly approved coordination with the MLX task, this agent-only rollout
and local integration. The MLX coordinator confirmed its live-transition and
shared-index hold. The existing paused controller advanced from generation 528
(`codex-mlx-upgrade`) to 529 (`codex-astra-interactive`) for the sanctioned
wrapper. No automation was resumed. Both remote main tips matched the local
bases above. The canonical Minime tree was clean before installation; 1,660
foreign Astrid paths were inventoried by hash and mode and retained unchanged.

A first read-only preflight omitted the worktree-to-canonical controller binding
and refused missing worktree-local control state. No install or signal occurred.
The supported `ASTRID_STEWARD_REPO_ROOT=/Users/v/other/astrid` binding then passed
the ordinary preflight without altering or copying controller state.

Fresh checks: 44 focused repair tests; 129 wrapper/controller/evidence tests and
40 subtests; complete Minime suite **2,300 passed, one skipped, 167 subtests**.
The complete rerun uses the unchanged qualified helper, synthetic fixtures and
stubbed providers. No Rust build, new model, provider request or cache cleanup
was required for qualification. Disk availability was 437 GiB.

The unchanged `paired_minime_handoff.py --agent-only
--install-reviewed-overlay` procedure installed the exact reviewed overlay,
retained old source bytes, waited its 185-second stability interval, verified a
stable idle/no-active-job boundary and signalled only old PID 60407 with SIGTERM.
All overlay paths were verified; exactly two paths differ in content from the
previous loaded inventory. No accepted job was cancelled or forcibly retried.

`agent-rollout-01.jsonl` reports success at `2026-10-18T20:53:21.888836+00:00`:

- Old PID 60407, native boot `7f1fb8d6a04f40aab474e21bbe54013e`, replaced by PID
  40182, native boot `b43bff0a973e4904b58a2eaec63f91a7`.
- OS process start: `Sun Oct 18 13:51:56 2026`; native agent initialization:
  `2026-10-18T13:52:11`. These are distinct lifecycle timestamps.
- All 101 source inputs match the frozen release; no reload requirement. Fresh
  `action_scheduler_v1` status is present, and the normal loop reached readiness.
- Pending NEXT was empty at shutdown. Seal
  `39e24fadc2a7aab68b5615ac6c12e7686b35486ead2abcf044a325e5327104dc`
  was acknowledged by restore record
  `142321378be5f21b058dcafafe8bb63df2b0d48e6d6fe599583398156084518c`.
  This proves preservation, not execution or a wish to remain idle.
- The temporary launch hold was released. Launch configuration, the unchanged
  immutable helper and all ten protected service PID/start identities match the
  pre-rollout record. Bridge PID 61069 and provider PID 3893 remain unchanged.
- Read-only initial telemetry advanced through timestamps 1477901512,
  1477903874 and 1477906250 ms, with fill ratios 0.7305844, 0.7107255 and
  0.7305304. This bounded observation is not a controller-settling claim.

Artifacts remain under the tranche root given above:

| Artifact | SHA-256 |
| --- | --- |
| `agent-rollout-01.jsonl` | `6bc08fd0dc46287fa5bb06fde546216c5c08295733d3cd7e80446f1822ce30c9` |
| `rollout-before.json` | `95c7ed258ab0aac73eb47ad3b3e344de4e1cba2be64bf3efae6fd4a5964f4fd6` |
| `rollout-after.json` | `e15669fac02eeba94682bf1506afc4d0d9ef051481988434d7d99ff655e00336` |
| `preintegration-minime-suite.xml` | `fc852c2efa723ae5fadd9c7f990a369f10092b4bc287533a4597b78d25fc565e` |

The after receipt binds loaded identities during initialization; final readiness
is established by the subsequent terminal wrapper receipt, not inferred from
that earlier snapshot. The original failed historical qualifier and all prior
evidence remain intact. No old regulation request was replayed, and no new
reflection was requested as a test of improvement. Natural uptake remains to be
observed separately from deployment success. Larger capacity and MLX work are
not included. The MLX coordinator was told to refresh old Minime process/source
bindings before any later maintenance.

## Local Integration and Window Closeout

One coordinator reviewed full staged diffs and committed only the owned paths,
then fast-forwarded both canonical local main branches:

- Minime `b0bedd98cd02a55a7a8a196a9614e810b1e9de03`: `CHANGELOG.md`,
  `minime_autonomy/runtime.py`, `minime_autonomy/action_scheduling.py`, and
  `tests/test_regulation_action_scheduling.py`.
- Astrid `a07a447dd6114bf4ea165031b9c59944d061583d`: `CHANGELOG.md`, this note,
  `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`,
  `scripts/reconcile_minime_launch.py`, and
  `scripts/test_reconcile_minime_launch.py`.

Both commits identify the Codex interactive coordinator in their provenance.
Final staged-state tests passed: 44 Minime request/admission tests and 19 Astrid
reconciliation/inventory tests. No executable bytes changed after qualification.
A synthetic Git fixture qualified the explicit target-index fast-forward before
using it with the already-installed source and shared changelog. The entire
canonical staged trees matched the reviewed destination commits, excluding all
foreign work. No stash, reset, broad add or history rewrite occurred.

`rollout-after-integration.json`, SHA-256
`44242056a84954354e7c3b19237a5e9367b3a0fade659a323848f7f91d2bb9e8`,
verifies that all 101 committed Minime input blobs equal the loaded release,
the fresh process is ready, the launch hold is absent, and the ten protected
process/configuration/helper identities remain unchanged. It verifies both clean
indexes, a clean Minime worktree and all 1,660 preserved foreign Astrid paths.
For the shared changelog, preservation removes only our exact committed added
block before comparing the original foreign hash; it does not pretend the
combined file is byte-identical to its earlier whole-file state.

The new agent has admitted ordinary work: this receipt observed one active job,
and fresh scheduler status at 13:56:34 PDT reported `primary_job_active` and a
pending-choice boolean without private payload. This is bounded operational
evidence, not proof that the new regulation-request branch has yet been chosen
or that an entry completed. Follow-up telemetry advanced to 1478211790,
1478214150 and 1478216516 ms with fill ratios 0.7117497, 0.73150706 and 0.7115212.

The controller returned to paused `codex-mlx-upgrade` ownership at generation
530; no automation was resumed. Indexed-tail Evidence Event Store verification
passes at sequence 1123184, head
`56b45504442dcf456a8b4f6676c135cb84ead875ff60602a85093979e4420667`,
with V1 immutable, no active lease and no active projection. The two remote main
tips still equal the original bases: these are local integrations, not pushes.
This documentation-only closeout changes no runtime, installed source or state.
Natural uptake and the separate capacity/MLX qualification remain future work.
