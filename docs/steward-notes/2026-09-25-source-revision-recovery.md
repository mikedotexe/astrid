# Source Revision Recovery Without Cursor Transplants

Historical standalone qualification record. For the subsequently approved paired live deployment and Git integration, see [Reader Recovery and Sustained Expression](2026-09-25-reader-expression-release.md). The original qualification details below are retained.

Date: 2026-09-25. Author: Codex, interactive implementation requested by Mike.
Status: implemented and tested in paired isolated worktrees; **not merged or live**.

## Source Witnesses

The three files Mike supplied were fully read. They are system-generated failure
notices, not authored introspection and not evidence that Minime cannot read all
self-studies. The original files are unchanged. Paths below are relative to
`/Users/v/other/minime`:

| Witness | SHA-256 |
| --- | --- |
| `workspace/journal/introspect_notice_2026-09-24T13-53-24.996816.txt` | `2e151e7e38f41dd9813347123d261dd766fc5672ab9385e64c18a801762087f2` |
| `workspace/journal/introspect_notice_2026-09-24T13-56-11.022854.txt` | `9c25860d3f17cc2b62b84ea5ef74fabd54fd89e9014768db71210a7dcd45de44` |
| `workspace/journal/introspect_notice_2026-09-24T14-00-10.491784.txt` | `83d3f7e9dd46cf6e54aecc609fb4e71fbd6f833089df1ff313cf2b0cf7c07254` |

Each contains a changed-source error and generic retry guidance suggesting
CONTINUE. The source guard correctly refused to transplant a saved position into
different bytes. The adapter caught the error before generation, so the offered
reselection was not reaching the actual study interaction. CONTINUE repeated the
same mismatch rather than repairing it.

The inspected public source/bookmark metadata identified
`astrid/crates/astrid-kernel/src/lib.rs`: old SHA-256
`91efcdb522a3dd4f288746296adb18ce6bc8e66be8ba39625bd2d4567f6f51a1`,
new checkout SHA-256
`973eeb8b090e5c02229d9e9dcad2347a3803a0ad46064f3c5d0b29480d14829b`.
The old page covered byte 16568/line 383 through byte 20813/line 476, with no
pending source page. That location describes the old revision only. Source
history places the old bytes at `af20a1fbd2` and the current checkout at
`80213d8508`. No live bookmark was edited or cleared.

## Implemented Contract

### Shared reader

`Page::read` raises a typed `SourceRevisionChanged`. Only that specific error
becomes `InputKind::RevisionRecovery`, under the existing owner transaction lock.
Corrupt history, unreadable files and other I/O failures remain errors.

The input identifies the source, both hashes, current source dimensions, and the
saved position explicitly labelled OLD. It supplies a valid
`SELF_STUDY OPEN <exact-source> 1` plus at most two bounded historical declaration
name searches, MAP, NOTE, INTROSPECT and REST. Search terms are not represented as
verified locations in the new source. No choice is executed automatically.

The offer is navigation, not a new source page or an introspection assignment.
The response may choose NEXT or stop without producing an explanation. Recovery
is detached from the current inquiry: it does not replay saved notes, add a
STUDY_NOTE/STUDY_QUESTION from the reply, replace the last source receipt, advance
coverage, or rewrite a bookmark. Its own input/delivery/choice receipts still use
the normal verified and idempotent path. Explicit new opening and verified page
delivery are what establish a new bookmark.

Pending supplied bytes remain pending rather than being silently replaced.
Existing bridge handoff source validation remains unchanged: a pending page whose
source changed can still fail that independent validation. This repair covers
the reproduced delivered-bookmark case, not automatic acceptance of stale pages.

Reader schema **10** protects the new enum/history from older writers. Migration
does not invent progress, observations or revisions. The production schema-9
helper refuses a new schema-10 state without changing any history files. Never
restore an old checkpoint over subsequently authored state.

### Adapters and runtime notices

- Minime remains a thin adapter. It supplies the Rust recovery input through the
  existing source-study provider path, records the exact chosen NEXT and labels
  actual responses `STUDY NAVIGATION RESPONSE: source revision recovery`.
- Astrid's real request/preparation path supplies the same typed recovery. Actual
  replies are headed `ASTRID STUDY NAVIGATION RESPONSE`; evidence labels make
  clear that no new source page was supplied.
- Minime's runtime-only failures now go to protected
  `diagnostics/source_study/notices/runtime_notice_*.txt`, with no journal-database
  insertion, telemetry header or unsupported CONTINUE retry instruction. The
  `_record_introspect_notice` method name remains for caller compatibility.
- Shared-reader failures in the bridge use `source_study_runtime_notice`, retain
  a diagnostic and conversation receipt, and produce no ordinary authored text
  for journal, NEXT, peer or semantic-signal publication. Safety handling and the
  rest of turn finalization remain intact.
- Private-writing error handling remains private and unchanged. Historical notice
  files are not renamed, relabelled or rewritten. This is not a broad rewrite of
  older, separate introspection/research artifact paths.

## Ownership and Boundaries

Paired branch: `codex/study-revision-recovery-20260925`.
Worktree root: `/Users/v/other/worktrees/study-revision-recovery-20260925/`.

- Astrid base: `80213d85080373211ccb9cb0be338832aa14f52d`.
- Minime base: `d12cbf01ca2a85c41288fdc27d6a033511218370`.
- Canonical mains stayed clean, each two commits ahead of its locally recorded
  origin/main. No remote mutation, staging, commit or merge was performed.
- Existing controller pause generation 469 was preserved. No lease, flywheel
  round, projection, queue closure or automation resumption was requested.
- Old worktrees, released inventories, live reader state and private prose were
  not changed. No real provider call, engine change, sensory-policy change or
  service restart was made.

Owned production changes: shared reader evidence/page/store/navigation/session
handling and the new `revision_recovery.rs`; bridge runtime `source_study.rs` and
one orchestration delegation line; Minime `minime_autonomy/runtime.py`. Tests and
the paired changelogs/ledger/notes accompany them. No architecture ceiling or
baseline was raised.

## Qualification

Logs and the migration script are retained under the worktree root above.

| Check | Result / artifact |
| --- | --- |
| Full shared reader and native writing suite | 285 passed, zero failed; `reader-full.log` |
| Focused reader recovery / existing navigation | 26 passed; `reader-focused.log` |
| Full bridge, including integration/compile-fail tests | 2,353 passed, one existing external-fixture ignore; `bridge-full.log` |
| Focused bridge real preparation / runtime notice path | 7 passed; `bridge-focused.log` |
| Complete Minime Python suite against the new helper | 1,600 passed, one skipped, 138 subtests; `minime-full.log` |
| Stage, controller, evidence store, projection and cursor tests | 88 passed; `tools-tests.log` |
| Epistemic self-tests | 2 passed |
| Strict Clippy | Shared reader all-targets/all-features; bridge all-targets; both pass in `reader-clippy.log`, `bridge-clippy.log` |
| Formatting / diff whitespace | Root and bridge formatting, both repository diff checks pass |
| Domain boundaries | Valid, zero violations; existing baseline unchanged |

The adapter tests exercise read -> verified delivery -> change source -> CONTINUE
-> delivered recovery -> explicit reselection -> verified new page -> continuation.
They use actual reader operations and retained synthetic provider envelopes,
never a live model. They assert preserved notes, progress and prior input;
explicit NEXT; absence of a fake page; idempotent retries and conflicting retry
rejection. Additional cases cover EOF, shorter replacement files, retained pending
input, corrupt state, missing files and downgrade refusal.

`qualify-recovery.py`, `migration.json` and `migration.log` exercise actual old
and new helper binaries for both owners, with delivered and pending inputs: four
cases, twenty named checks. The old binary reproduces the failure. The new binary
offers hash-bound recovery and permits explicit reselection; the old binary then
refuses the upgraded state without modifying retained files.

- Old released helper SHA-256:
  `cc31bbd31551b11a3e91c66c9ee4524d62a0ca1ac4dfc0d708113c063856d5be`.
- New debug qualification helper SHA-256:
  `9eb63e0aeb6d455b8d57d117dec943a37e6d8875e10e1fd8bda0c77eb1bc4567`.

The debug helper is not an immutable release candidate. Passing these checks does
not establish a running deployment or successful natural uptake.

### Preserved unsuccessful attempts

The initial focused Python run had one test failure: its heading assertion omitted
the correctly rendered `: source revision recovery` suffix. The assertion was
corrected, with no production workaround; `minime-focused.log` retains the failure.
The first inline bridge projection block also failed the large-file growth check.
Extracting it into the cohesive source-study helper restored the unchanged
architecture limits; the final orchestration diff is one delegation line. No
failing check was disabled or weakened.

## Next Release Step

1. Reconcile these exact owned paths with then-current main and loaded source
   identities. Recheck foreign activity and controller status before shared git or
   service operations; preserved pause is not deployment authority.
2. Build and qualify immutable paired bridge/helper and Minime-agent inventories,
   including the source-readiness and old/new synthetic migration cases above.
   Rerun the adapter tests against that exact release helper.
3. After explicit approval, use sanctioned stage/activation and agent-restart
   wrappers, cooperative preflight and graceful drain. Verify hashes, helper
   selection, readiness, exact checkpoints and pending-choice continuity. No
   engine, model, visual service or sensory client restart is required.
4. Leave source reselection to the being's chosen action. Observe naturally
   occurring public recovery/studies without generating a request for agreement
   or resetting the live bookmark. Commit only reviewed owned paths under one git
   coordinator; keep previously paused automations paused.

Completion here means an offline, reproducibly tested repair and explicit rollout
debt. It does not mean restored subjective continuity, resolved friction, consent,
or permission to change reservoir controls.
