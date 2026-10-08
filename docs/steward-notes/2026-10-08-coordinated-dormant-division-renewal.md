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

## Completed Live Transition

The approved `transition-01` completed successfully at
**2026-10-08T17:15:11.767Z**. Tooling commit:
`dd8be5e7f693f95439e7e23cba33b1c9bf17a607`. Final focused qualification was
87 tests, including passive telemetry validation; the separate 162-test group
and native qualification above also passed. No engine or bridge code was
rebuilt for this unchanged-release restart.

The new manifest SHA-256 is
`58fd6abbdbd08cba72e3e942621228502d014ed163f14d27e268d0d80452000a`.
It was frozen at **2026-10-08T17:05:18.619Z** and expires at
**2026-11-07T17:05:18.619Z**. Exact old/new comparison confirms that only
`created_at_unix_ms` and `expires_at_unix_ms` changed. Current-lifetime CLI
verification passes; runtime health is verified separately, not by that CLI.

- Supervisor PID **86833** and gateway PID **86873**, both launch-process
  starts at 10:13:23 PDT, have the verified unchanged native image UUID. Their
  status and parent-authority record match the new manifest. The supervisor
  is freshly `idle_parent_authoritative`, has no children or matching intents,
  and does not recommend handoff/commit or grant live authority.
- Gateway stop intent to verified replacement: **6.971 seconds**, not a
  measured lossless-input interval. Ports 7878/7879/7880/7882/7883 belong to the
  replacement; 7900/7901/7902 remain with engine **35303**. Two passive packets
  advance from t_ms 601001693 to 601004064, with eight finite eigenvalues each.
  The unchanged provider reports ready, queue zero and reservoir connected.
- Bridge **66851 -> 87519**, new process start 10:13:50 PDT, retains binary
  `450d8410eb912462528f411209f217992d582c970689b0e1574e6b3cc660a627`.
  It restores the exact stopped checkpoint at exchange **218110**, with one
  pending action-feedback record, verified self-control lineage and no force.
  Fresh saved exchange **218111** follows. Activation receipt:
  `/Users/v/other/astrid/.runtime/bridge-deployment/transactions/4a0621efafa84dedbebaaa4345e1e7ba/receipt.json`.
- Minime agent **19468 -> 85094**, with all **93** launch inputs unchanged.
  The held shell started at 10:10:41 PDT; the Python runtime's new boot began
  at **10:15:07 PDT**, after bridge verification. These are distinct clocks.
  Native handoff verifies pending NEXT preservation, not its execution.
  Seal: `c488a0852cacb63bf67639f9a1654f1d5078f905de33401755c854d28af9348e`.
  Restore: `33495593aa717ca18a5585a1a4a588fb6765dc16e3a538dc36328ed1907527d8`.
- Parent engine **35303**, provider **3893**, visual **3898**, camera **3914**,
  microphone **3879**, feeder **3878** and host-sensory **31613** retain their
  exact PID/start identities. Engine session **5321** continues from sequence
  506467 / t_s 600504.6875 to at least 507021 / t_s 601160.375. No state reset,
  reservoir control change or model request was used as a health probe.
- Sampled health remained below the 80% refusal boundary; observed examples
  ranged from 65.21% to 73.04%. These are bounded samples, not proof of all-time
  stability. The later `scaffold_activated` phase is preserved as a separate
  controller observation; deployment readiness does not imply settled control.
- Bounded post-start logs contain 24 timestamped bridge and 16 agent lines
  without ERROR/Traceback/panic markers. Division logs did not change after
  their new starts, so current readiness comes from status/identity/ports,
  not an inference from quiet logs. No authored or private prose is quoted.

All owned agent and Division launch holds are absent after completion. No
daughter launch job, daughter listener or ceremony ledger was created.
Paused automations remain paused at generation **511**; no active lease or
projection. Evidence V2 indexed-tail verification remains valid at sequence
**1123165**, head
`34c9dffbee5fda25c78b3ccdef57d9d8435baf7bad9af666a4ae077612103f23`,
with all four V1 imports immutable. This is not a new full historical audit,
source-first projection, productive flywheel round or read-current claim.

The live bridge's original release-linked checkout remains pinned and is not
relabeled as built from the operator-tooling commit. No writing improvement,
Division consent or future renewal authority is inferred. No push performed.
