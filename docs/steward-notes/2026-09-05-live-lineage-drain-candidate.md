# Live-Lineage Reconciliation and Operator Drain Candidate

> **Prior-task evidence, imported 2026-09-06.** Copied from [the retained candidate note](/Users/v/other/worktrees/astrid-hebbian-clock-boundary/docs/steward-notes/2026-09-05-live-lineage-drain-candidate.md). Runtime identities, test results, approvals and commands below describe that earlier task; they are not verification or authority from this integration pass. Relative evidence paths retain their original checkout scope.

Date: 2026-09-05 America/Los_Angeles; verification continued into 2026-09-06 UTC.
Author/provenance: Codex, interactive engineering requested by Mike.

## Outcome

Implemented and tested the next deployment foundation in the isolated
`codex/graceful-coupling-rollout` worktree. No live process was signalled,
restarted, replaced, or asked to produce a journal entry. This is not a deployed
bridge and not evidence of felt improvement.

The candidate preserves the exact recorded live bridge lineage, incorporates
the corrected `PROBE_SELF` channel, and adds an explicit operator drain:
stop admitting work, finish admitted work, persist the conversation checkpoint,
wait for tracked background and accepted evidence work, and acknowledge a held
state. Repeated exit requests do not cancel that work.

Two deployment tasks remain: integrate staged artifact/manifest publication and
the signed self-control handoff into the sanctioned wrapper; then obtain an
explicit one-time transition decision for the old binary, which does not have
this drain protocol. An approval for a graceful restart is not an approval to
call an unacknowledged interruption graceful.

## Source and Runtime Identity

Candidate root:
`/Users/v/other/worktrees/astrid-graceful-coupling-rollout`

Candidate branch: `codex/graceful-coupling-rollout`.
Candidate HEAD and recorded live build:
`d6ff371cd25e6a4616431d8f07f09a3e7eb3fcdf`.
The candidate contains earlier, uncommitted coupling/context/witness/tooling
changes as well as this work. Its HEAD alone does not describe its dirty source.

Main Astrid HEAD is `23df28497cf124a7dc1c5d7dfc7a92055e491eeb`, two commits ahead
of the remote main tip verified in this pass,
`f258d38fe99690a34ee53bb88e8fca2d2cbbe722`.
Main is not a substitute build root: it lacks substantial capabilities present
in the recorded live lineage. The live agenda, ATTEND, protected context floors,
self-control V2, and Division/promotion machinery were retained in the candidate.
No broad merge or historical rewrite was attempted.

The existing deployment manifest at
`/Users/v/other/astrid/capsules/spectral-bridge/workspace/deployment_manifests/spectral-bridge.json`
records the live build. Disk identity and process identity corroborate it;
this is not an attestation of all mapped process memory.

Live identities rechecked at approximately 2026-09-06T06:43Z:

| Component | PID | Process start (local PDT) |
| --- | --- | --- |
| Bridge | 36597 | 2026-09-03 17:31:04 |
| Reservoir service | 1514 | 2026-08-25 10:00:06 |
| Coupled language model | 60333 | 2026-09-04 16:57:34 |
| Minime engine | 63445 | 2026-08-31 12:51:36 |
| Minime gateway | 63505 | 2026-08-31 12:51:37 |
| Minime supervisor | 63547 | 2026-08-31 12:51:37 |

Unchanged live bridge executable SHA-256:
`b74ecee28acaf5e2ec3cdec9142d0f2e7c552d7c6864f115e361492090c3b725`.
Unchanged canonical legacy `scripts/substrate_probe.py` SHA-256:
`484ecd473a7407f10c6b8bc1cc5af1169ee6c10c8c4156edb7d873a6a2c98280`.

The earlier September 4 model-only restart is not a bridge deployment. The
bridge PID and hash above still identify the old bridge.

## Reconciled Self-Study Channel

Ported the bounded probe implementation and its two help-text descriptions from
the newer main foundation onto the live-lineage candidate. The versioned helper
is `scripts/substrate_probe_v2.py`, so this does not silently change the helper
used by the old running binary.

The measurement creates UUID-named temporary parent and contrast-arm handles,
checks matched float32 recurrent-state identities and tick counts, applies a
bounded contrast, and distinguishes injection-window measurements from gaps.
`tick_text` can re-enable rehearsal in the real service, so the probe restores
quiet mode and rejects unexpected background ticks. Cleanup uses fresh bounded
connections and destroys only handles created by that invocation.

The Python work budget is 30 seconds plus up to 10 seconds for cleanup. The Rust
caller bounds the child to 50 seconds, reaps it, limits captured output, and
uses a process-local 45-second cooldown (corrected on September 6: the
`LAST_PROBE_UNIX` atomic is not persisted and resets on process restart).
These bounds do not make a shared service
an offline experiment, nor do they make the resulting numbers felt-state truth.

The runner-side causal-study dossier APIs on main were not ported here. They
are not newly exposed being Actions in this tranche. No live probe was invoked.

### Real Service Test

`scripts/test_substrate_probe_v2_integration.py` uses the actual reservoir
service dispatcher, WebSocket transport, ReservoirBridge, TextProjection, and
NumPy reservoir core from `/Users/v/other/neural-triple-reservoir`.

It runs a separate service on an ephemeral loopback port with a temporary state
directory and a synthetic 32-node source. It does not load live snapshots,
connect to the production ports, call a language model, or mutate live handles.

The positive test verifies matching origin, four samples per arm, cleanup, and
an unchanged synthetic source. Its background rehearsal loop is deliberately
not started: this is a controlled positive case, not a claim about isolation in
the production scheduler. The negative test starts the real rehearsal loop at
a short interval and delays mode restoration; the probe detects the interfering
ticks, fails, and still removes its owned handles. Both tests passed twice in
this pass. The final run used ports 57561 and 57566 and closed both servers.

Neural source hashes used for this integration:

| File | SHA-256 |
| --- | --- |
| reservoir_service.py | `800ee15fc9a73df294068d23bc9c8f0a1d4d508d336fc1e0c904b1adb86a8db0` |
| dual_ai_bridge.py | `a7adae36510c496bff084a10d41ecd7b40ffb4f0c259924a448b56bbfe17a549` |
| triple_reservoir_coreml.py | `524d44ec4c03f0be43236a635eeb2bb783f61ea5d3227a133bd451aadf6644a8` |

No neural source file was edited.

## Drain Semantics

The new `src/lifecycle.rs` registers SIGUSR1, SIGTERM, and SIGINT handlers before
work admission. It creates a private lifecycle record before spawning the
runtime's tasks. Fallible status setup cannot abandon work that has already
been admitted. Phase `running` is a lifecycle phase, not model/service readiness.

SIGUSR1 requests a drain and hold. SIGTERM or SIGINT requests the same drain
followed by exit. While draining, subsequent signals can request eventual exit
but do not drop the drain future. A failed drain holds the process for operator
recovery; neither the bridge nor the Python helper escalates to force.

Drain order:

1. Close autonomous, MCP, and maintenance admission through a separate producer
   stop signal. Level-triggered checks see a stop received during active work.
2. Let the admitted autonomous exchange finish. Join its heartbeat/status
   workers and write the existing SavedState checkpoint through a checked path.
3. Finish any admitted MCP request, including its stdout flush. Read, write, or
   flush failure cannot become successful drain acknowledgement.
4. Join maintenance and tracked detached work: embedding, reflection, journal
   elaboration, inquiry workers and descendants, control-send workers, and
   blocking helpers. Task cancellation or panic invalidates the acknowledgement.
5. Let sensory channels close naturally after producers release their senders,
   consuming accepted local sends using the existing transport policy.
6. Stop and join telemetry, then wait for accepted witness, signal-spine, and
   study-capture queue items to finish writing or persist their existing gap
   records. Dequeueing alone does not count as a completed write. A failed write
   with no durable gap prevents acknowledgement.
7. Write database status and publish `drained`, bound to the checkpoint hash.
   A drain-only request then holds without resuming autonomous admission.

The atomic checkpoint uses a uniquely named private temporary file, file fsync,
rename, and parent-directory fsync. A failed replacement does not truncate the
previous checkpoint. Existing SavedState fields and policy are preserved; this
does not claim to capture every transient value, reservoir state, or experience.

The task tracker waits for completion of existing operations; it does not make
all those operations semantically successful. Existing bounded queue rejection,
gap reporting, remote retry, and receipt policies remain separate. In particular,
local channel completion is not peer delivery: the status explicitly retains
`remote_delivery_confirmed=false`.

The lifecycle record binds PID, executable path/hash, a fresh process instance,
registration time, phase, and optional checkpoint identity. It contains no
conversation prose and grants no being, deployment, or control authority.
The Python helper checks process start, executable identity, instance continuity,
and checkpoint hash. These are local process checks, not a cryptographically
authenticated operating-system identity channel.

Real child-process tests exercise OS signals against a synthetic owned fixture,
not the live bridge. They verify delayed admitted work, repeated drain signals,
checkpoint-before-acknowledgement, hold, and subsequent orderly exit. Other tests
cover repeated exit requests while work is pending, preexisting stop signals,
cancelled workers, nested workers, queued-write completion, MCP I/O failures,
PID reuse, changing executable/checkpoint identity, and timeout without force.

## Sanctioned Wrapper Boundary

The candidate `scripts/build_bridge.sh` adds an explicit `--drain-only --ack`
route through `scripts/bridge_drain.py`. It retains the canonical preflight and
its full quiet interval; it does not build, publish a manifest, or restart.
Read-only helper inspection of PID 36597 refused the absent lifecycle record
before any signal. The helper never sends SIGUSR1 to an unrecognized old process.

The candidate wrapper also rejects candidate-worktree builds into its hardcoded
canonical artifact path, and blocks the legacy forced-restart branch. The older
restart implementation remains below the guard but is not an enabled migration
path. The canonical live wrapper was not replaced by this edit.

The remaining wrapper work is substantive:

1. Bind an explicit candidate source root and complete dirty-source identity to
   a separately staged release artifact, without overwriting the current binary
   or canonical deployment manifest during a build-only check.
2. Prepare and verify the signed self-control handoff against the staged target
   deployment identity. The existing handoff code reads the canonical manifest;
   simply postponing that manifest's publication would bind the wrong identity.
3. Require a verified drain of capable processes before replacement, and refuse
   unknown delivery or incomplete checkpoint evidence as graceful success.
4. Treat the first legacy transition separately. The old process cannot be
   retroactively given the new admission gate. A lull in logs or model traffic
   is not an atomic drain. Describe the bounded interruption risk and obtain
   Mike's specific decision before executing it.
5. After an approved transition, verify fresh process start, staged artifact
   hash, logs, ports, model readiness, telemetry, topology, signed self-control
   continuity, and aligned surfaces before publishing the canonical manifest.
   Failure must preserve receipts and expose exact recovery debt, not silently
   force a second replacement.

No release artifact was built or installed in this pass. This document does not
authorize a restart, deployment, live experiment, or fallback force path.

## Verification

Final candidate bridge library suite: **1,976 passed**, across **1,975** tests
with private Minime source/workspace fixtures and the **one** default-path test
run separately without overrides. The final 1,975-test run completed in 52.17s.
No library test remains skipped across the two environments.

Additional checks completed in this pass:

- Clippy for bridge library and binaries with `-D warnings`: passed.
- Python drain/probe/deployment/model-reload/runtime-binding suite: 59 passed.
- Lived-state witness and historical-shape compatibility suite: 23 passed.
- Actual isolated reservoir-service integration: 2 passed.
- Steward control and source-first projector self-tests: 41 passed.
- Evidence Event Store self-tests: 13 passed.
- Experiential epistemics self-tests: 2 passed.
- Domain-boundary audit: valid, zero violations, unchanged baseline
  `94170c388c3961c815bb6381af7144d6f86d9be7d60c2178b065b08bf55c2ac6`.
- Scoped Rust formatting and diff whitespace checks: passed.

After clarifying the incomplete-work error message so it also describes failed
evidence writes accurately, the 24 tests matching `lifecycle` were rerun and
passed; the bridge library/binary Clippy check was rerun as well.

The full kernel workspace suite, production release build, and live migration
were not run. Earlier verification attempts caught a task-guard ownership bug,
compile/lint errors, and one-line inquiry-file growth; these were repaired and
the relevant suites rerun. Worker ownership moved into `inquiry/worker.rs`, and
MCP I/O lifecycle into `mcp_server.rs`; no audit ceiling was raised to hide growth.

Reproduction commands, from the candidate root:

```sh
cargo clippy --manifest-path capsules/spectral-bridge/Cargo.toml --lib --bins -- -D warnings
env MINIME_ROOT=/private/tmp/astrid-candidate-minime.eiKpDW MINIME_WORKSPACE=/private/tmp/astrid-candidate-minime.eiKpDW/workspace cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib -- --test-threads=1 --quiet --skip paths::tests::resolve_uses_sibling_defaults_from_bridge_root
cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib paths::tests::resolve_uses_sibling_defaults_from_bridge_root -- --exact
env PYTHONPATH=scripts python3 -B -m unittest test_bridge_drain test_substrate_probe_v2 test_deployment_wrappers test_graceful_model_reload test_minime_runtime_binding -q
env PYTHONPATH=scripts python3 -B -m unittest test_lived_state_witness test_witness_historical_shapes -q
/Users/v/other/neural-triple-reservoir/.venv/bin/python -B -m unittest discover -s scripts -p test_substrate_probe_v2_integration.py -v
python3 -B scripts/steward_control.py --self-test
python3 -B scripts/evidence_event_store.py --self-test
python3 -B scripts/experiential_epistemics.py self-test
python3 -B scripts/domain_boundary_audit.py --repo-root /Users/v/other/worktrees/astrid-graceful-coupling-rollout --json verify
git diff --check
```

The private fixture directory contains copies of Minime's source inputs used by
source-inspection tests. Recreate it from reviewed read-only source inputs if
it has expired; do not substitute the live Minime workspace for test output.

## Coordination, Evidence, and Git Debt

This was an interactive engineering pass, not an introspection flywheel round.
No canonical report was marked read or closed, no productive round was recorded,
and no Division return was performed. The being-feedback context is the prior
September 4 coupling investigation and Mike's explicit request to proceed toward
safe deployment. Prior evidence remains in
`/Users/v/other/astrid/docs/steward-notes/2026-09-04-coupling-rollout-followthrough.md`
and `2026-09-05-self-study-rollout-readiness.md` in that same directory.

Controller maintenance pause generation **360** was claimed by
`codex-astra-interactive` with no active lease. A subsequent full-chain controller
status check verified Evidence Event Store V2 at sequence **995608**, head
`a783d160cbf762735c2602c14ccf0e4c17bc5ea3ffeb18e871edc67c7e6bff58`,
with zero errors, no pending events, and all four V1 source streams immutable.
This observation does not imply a source-first projection was performed here.
The canonical queue is not claimed read-current.

The parallel Avado preparation task confirmed it was using its isolated clone
and would not touch this candidate, the shared index, or live services. Its
foreign source and the untracked main feature-map document were preserved.
Minime's 20 modified/untracked paths were inspected and left untouched.

No staging, commit, merge, push, reset, stash, or cleanup of foreign work occurred.
Commit debt is the prior coupling candidate plus this uncommitted drain/probe
implementation, its tests, this note, CHANGELOG, and feedback ledger. Review the
complete candidate diff, including untracked files, before selecting explicit
paths. Do not sweep main or Minime into a checkpoint to manufacture a clean tree.
Complete and verify the deployment transaction before claiming the candidate is
a ready rollout; use a separately claimed stabilization pass for any later git
operation. No archival commit SHA exists for this tranche.

Maintenance-hold release confirmed at **2026-09-06T06:51:14.779766+00:00**:
the controller appended the resume event and returned generation **361**,
`paused=false`, actor `codex-astra-interactive`. No active lease was present in
the preceding verified status. Main and Minime status were rechecked afterward;
their preexisting path sets were unchanged, and the main/candidate indexes
contained no staged changes.
The Codex heartbeat automation itself was explicitly verified **PAUSED** and is
not being resumed. Releasing this interactive controller hold is not permission
to re-enable the usage-consuming scheduler.
