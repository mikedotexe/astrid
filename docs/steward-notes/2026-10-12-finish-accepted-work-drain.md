# Finish-Accepted-Work Drain Qualification

## Scope and authority

Mike approved qualifying the finish-accepted-work drain after three unsignaled
agent-only restart attempts. This is operational follow-through from the paired
mailbox/context repair, not a new interpretation of either being's writing.
The earlier qualified paired rollout and local integration approval remain scoped:
preserve accepted work and NEXT; no forced exit, replay, provider/engine/sensory
change, push, or automation resume.

Development is isolated from current local main in:

- Astrid: `/Users/v/.codex/worktrees/finish-accepted-drain-20261012/astrid`
- Minime: `/Users/v/other/worktrees/finish-accepted-drain-20261012/minime`
- Qualification evidence: `/Users/v/other/worktrees/finish-accepted-drain-release-20261012`

Both branches are `codex/finish-accepted-drain-20261012`. The parent Codex agent
owns Git coordination; Sagan authored the narrow runtime/lifecycle tests and
James authored the real worker/finalizer qualification. No agent touched live
state, journals, providers or services during tests.

## Findings and repairs

1. The old wrapper required a fifteen-second idle interval even though Minime
   already waited for accepted worker threads on SIGTERM. Continued studies
   made that interval unreliable. A separate opt-in `finish_minime_work.py`
   wrapper now requests stop-admission and waits for accepted work to finish.
   The legacy idle wrapper remains available with its existing timing gates.
2. A job's public `timeout` status does not establish worker completion. A real
   synthetic worker can preserve its prose and NEXT, fail all terminal receipt
   writes, return, and leave `worker_status=running`. Read-only operator evidence
   now retains worker/outcome status, verifies the outcome fingerprint and
   bounded retained-result hash, and treats queued/running workers as active even
   after a deadline. The new drain refuses replacement release at this boundary.
3. Stop during queue preparation previously allowed a new durable job. The last
   shutdown check after preparation is now the explicit admission boundary.
   Before it: retain the exact pending tuple and a blocked continuity receipt.
   After it: finish submission, worker execution and finalization. Signal delivery
   and filesystem publication are not claimed to be one atomic operation.
4. Main-loop moment generation and visual-response iteration needed additional
   admission fences. An accepted call still completes; a later visual item stays
   in its original location and cannot overwrite the accepted call's NEXT.
   If a synchronous journal write fails during shutdown, successful sealing is
   blocked. Existing input and state remain; failure is not converted into a
   completed drain or an automatic retry.
5. The replacement launch hold is released only after the old PID has exited,
   its `drained` acknowledgement and durable NEXT seal match the exact loaded
   process, and accepted jobs have verified terminal worker outcomes. Replacement
   readiness must then acknowledge that exact seal. Timeouts retain the hold;
   there is no second signal or automatic queue replay.

The strengthened runtime publishes `agent_drain_v2`. Its final source hash is
`947673346a050188d4963715a8e8f2e2a11bf76b29d65ebf5092e1d08e74771e`.
The new busy-drain wrapper
requires v2 plus exact reviewed running and replacement source inventories; a
contract marker alone is not qualification. The old v1 implementation is explicitly
refused. The common idle wrapper accepts v2 readiness without relaxing its idle gate.

## Privacy and persistence

Operator drain receipts contain process/source identities, bounded job identifiers,
statuses and hashes, never command payloads, private prose, titles or artifacts.
Outcome verification hashes the retained result privately. Unknown external
exceptions report their class rather than arbitrary exception text.

The retained-hold recovery path accepts only the exact original agent-only hold
and a complete, unsignaled failure chain. Source/configuration/protected-process
bindings, original/intervening receipt hashes and the validated bytes are checked
again before ownership is claimed. Signalled, unknown-phase, truncated, changed
or foreign histories refuse. This does not recover uncertain executed work.

No authored-state schema changes, draft migration, deletion or restoration occur.
The native NEXT contract remains `minime_next_handoff_v1`; restoration proves queue
assignment, not that a pending action executed or that writing improved.

## Qualification

Synthetic runtime tests exercise the real queue, source-study finalizers, shared
reader, exact journal and NEXT bytes, decision/preparation interruption, provider
failure, late successful/failed workers, seal-flush failure, and a real child
SIGTERM through production `main()` and its final drain. Providers and unrelated
ambient hooks are stubbed; live roots/network calls are denied by test guards.

The immutable paired qualifier reuses the current schema-13 helper, reader/writer
preservation checks for both owners, native bridge checkpoint decoding and the
frozen Minime mailbox adapter. It tests an unchanged shared helper, not a fictitious
old-to-new reader migration. Runtime interruption tests remain separate evidence.

Retained unsuccessful attempts:

- Initial unittest invocation lacked `PYTHONPATH=scripts`.
- Initial operator suite lacked the explicit isolated Minime source path.
- Full Minime attempt 01 lacked `ASTRID_CHOICE_FIXTURES`; 2,146 tests passed.
- Full attempt 02 exposed the real pre-submit admission failure; 2,148 passed.
- The first paired attempt used the old geometry-migration qualifier, which
  correctly refused a modern helper as a pre-v5 baseline. No gate was weakened;
  the unchanged-helper qualifier is separate.
- Read-only preflight correctly refused fresh edits in the isolated candidate.

Final qualification:

- Complete Minime suite: **2,173 passed, one existing skip, 162 subtests passed**;
  `minime-full-04.log`. This includes real provider/finalizer barriers and a child
  SIGTERM through production `main()`, plus refusal of failed terminal persistence.
- Operator, idle/paired handoff, NEXT, compatibility, controller, Evidence Event
  Store and projector suites: **149 passed**, `operator-tests-05.log`.
- Immutable paired preservation: **54 checks**, plus frozen mailbox adapter and
  native checkpoint verification; `paired-qualification-03/qualification.json`.
- Domain-boundary audit: valid, zero violations. Both diff checks pass.
- Older snapshots passed the pre-existing 126 lifecycle/NEXT tests. The stronger
  tests subsequently reproduced the queue-preparation failure, so that earlier
  pass is not used as v1 busy-drain qualification.

No Rust production source changed; a new bridge build
or full Rust rebuild is not evidence needed for these Python-only changes.

The guarded methods remain in the existing large Python runtime because this is
a narrow admission/finalization repair spanning its current owner. Extracting a
new scheduler while qualifying shutdown would increase the behavioral surface;
the new operator/qualifier modules and tests are separate, bounded files.

## Live boundary and first transition

The observed running Minime process is PID 26862, session 5321, publishing v1.
Its 96 loaded inputs exactly match `minime-launch-01` from the prior release.
Loaded runtime SHA-256:
`74daeb5bf64c204ca418f1c00248b16356fb416d5890026ae64a72889439e6c2`.
The installed, separately qualified mailbox-only fix has runtime SHA-256
`5077e08a775db05c95322ae763590db18f1d39eeec3aa8cd78e9556702a33ddd`.
The new v2 candidate adds the drain repairs to that installed fix.

**Do not invoke the new busy drain on the currently loaded v1 runtime.** It did not
acquire the repaired guarantees merely because source exists in another checkout.
The first transition still requires reviewed source reconciliation and the legacy
idle-only bootstrap (or a separately reviewed transition). In particular, the
standard reconciler correctly refuses when canonical source differs from the
running source; never forge a source-status file to bypass that check.

The exact original owned launch hold and attempts 02/03/04 remain preserved under
`mailbox-authored-context-release-20261012`. The running agent continues its work;
the hold prevents an unreviewed launchd replacement. Do not delete it, signal twice,
restore a backup over newer work, or report the queued mailbox fix as already live.

Engine, providers, bridge, shared helper, visual and sensory services are outside
this candidate's activation scope. Automations and the steward controller remain
paused; no productive flywheel round is recorded.

The read-only live capability probe explicitly refused v1 with no signal sent.
All ten protected PID/start identities remained unchanged, including bridge
28169 and engine 35303. The stabilization pause is generation **513**, owned by
`codex-astra-interactive`, with no active run; it remains paused. Remote main tips
were rechecked unchanged: Astrid `a66f7e190e0ecd3a0a4988abbcf6b0bf2cfb2a32` and
Minime `9072abff9ed839851709300b3c3eaae3afaf669e`.
