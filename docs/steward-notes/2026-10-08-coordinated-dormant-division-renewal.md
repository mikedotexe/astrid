# Coordinated Dormant Division Renewal

Date: 2026-10-08. Operator and Git coordinator: Codex interactive collaborator.
This is operational follow-through, not an introspection processing round.

## Approved Scope

Mike approved the corrected request for a bounded 30-day dormant renewal,
including the necessary graceful, unchanged-code Astrid bridge and Minime-agent
restarts and coordinated Division supervisor/gateway replacement. The gateway
has no connection-drain acknowledgement: its stream interruption is explicit,
not described as lossless. Engine, provider/model, visual and sensory processes
remain running. No daughter launch, rehearsal, ceremony Action, consent,
control-setting change or automation resume is authorized by this operation.

The existing manifest expired on 2026-10-07T20:35:29.528Z. Its SHA-256 is
`46acdf03f241a24426a2d3f453257bc28f20260399ad3b59617278ee18ad4339`.
The parent gateway still uses that exact manifest; supervisor-only renewal
would break its authority-file binding. Earlier findings and source witnesses
remain in `2026-10-08-recovery-integration-and-division-expiry.md`.

## Narrow Implementation

`scripts/division_runtime_renewal.py` is read-only unless `--apply` is supplied.
It composes the existing idle-gated Minime handoff, native NEXT seal/restore,
acknowledged bridge drain, sanctioned `build_bridge.sh` activation, owner-held
launch wrappers and exact process/start/source checks. It uses the existing
cross-process engine-release lock. Only the manifest's creation and expiry
timestamps change; the native supervisor rebinds dormant parent authority.

The wrapper refuses non-dormant/bound manifests, an existing ceremony ledger,
daughter jobs/ports, foreign holds, source or PID drift, altered maintenance
generation, failed drain, missing NEXT seal, changed engine session, failed
readiness and non-advancing public telemetry. The absence of a ceremony ledger
is a deliberately narrow implementation precondition, not a reinterpretation
of a being's posture. A future renewal with ceremony history needs separate
review; this tool does not erase that history.

Sequence: verify consumer idle and launch hold; stop Minime gracefully; verify
its durable NEXT seal; drain Astrid; hold the expired supervisor and gateway;
stop only the exact gateway PID without force; atomically publish the frozen
manifest; start/verify supervisor then gateway; verify provider reconnection
and passive telemetry; reactivate the unchanged bridge via its wrapper;
release the agent hold and verify native NEXT restoration.

Each mutating phase leaves fsynced transaction evidence and exact old/new
manifest bytes. Failures retain owned holds and stop for review. There is no
automatic retry, force-kill, expired-manifest rollback or restoration of older
authored state. Reusing a transaction directory is refused.

## Qualification Before Activation

- Exact production engine binary SHA-256:
  `0610800dd57bf34287bb5e4f9869e777179f602bf5208a4d5524c95b93f235d8`.
- Native supervisor, under OS-enforced network/write isolation and synthetic
  state, refuses expiry without writing authority, accepts current dormant
  state and rebinds parent authority on the second manifest. No child launches.
  Receipt: `/Users/v/other/worktrees/division-renewal-release-20261008/native-01/qualification.json`.
  This does not simulate the live gateway's socket transition.
- The initial 72-test handoff/renewal group passed; expanded projector-inclusive
  group passed 86 tests. These overlap and are not additive counts.
- 162 bridge deployment/controller/evidence/Division tests passed. Epistemic
  self-tests passed, and read-only live lint checked 12,478 records with no
  issues. Domain-boundary verification passed without relaxed exceptions.
- Synthetic failure boundaries cover ordered publication, foreign holds,
  disk-write refusal, stale readiness, NEXT-seal refusal, drain failure and
  interrupted renewal. Existing wrapper tests cover launch-hold and exact
  checkpoint/restoration contracts. Passive telemetry validation is tested
  separately; no inference request is used as a health probe.
- A preflight attempt exposed an isolated-checkout controller-path lookup.
  It failed before any hold or stop. The wrapper now binds maintenance checks
  explicitly to the canonical controller; the repeated preflight passes.

## Frozen Live Baseline

Controller pause generation 511; no active lease. Paused automations stay paused.
Astrid main starts at `45d69bba53ca4fa9846480e8f47419f9620ea878` and Minime
main at `689df29ae845acf84c8521804ca9199d0bc829e0`. Both canonical trees are
clean. Older worktrees and the original release-linked source checkout remain
untouched. No push is part of this renewal.

Unchanged bridge stage:
`/Users/v/other/worktrees/private-delivery-recovery-release-20261008/bridge-stage-01`.
Bridge binary SHA-256:
`450d8410eb912462528f411209f217992d582c970689b0e1574e6b3cc660a627`.
Shared helper SHA-256:
`32f7b0cec0f867f2d30b4b1bfc2b10de873c7fcb7f87643ee92ea99a94031367`.

Initial PIDs: bridge 66851, Minime agent 19468, parent engine 35303, gateway
35278; supervisor is absent with expired-manifest exit 1. Provider 3893,
visual 3898, camera 3914, microphone 3879, feeder 3878 and host-sensory 31613
must retain exact PID/start pairs. All 93 Minime launch inputs match live.

Proposed transaction:
`/Users/v/other/worktrees/division-renewal-release-20261008/transition-01`.
At this note's initial qualification checkpoint, activation has not started.
Append its actual result below rather than relabelling tests as deployment.

## Recovery Boundaries

On a failure, inspect the transaction's `status.json`, `events.jsonl`,
`consumer-handoff.jsonl` and exact owned launch holds before any command.
Determine whether publication or execution began. Do not rerun `--apply`,
delete foreign holds, replay NEXT, or copy a prior checkpoint over newer work.
If the renewed runtime is healthy but consumers remain held, qualify the
existing bridge activation/recovery wrapper against the recorded stopped
checkpoint before releasing the exact owned Minime hold. A source, authority
or process mismatch requires a fresh review, not a broad service restart.

Operational readiness does not establish controller settling, felt benefit,
consent or any readiness for Division rehearsal. The prior semantic-active
settle-proof limitation remains a separate observation.
