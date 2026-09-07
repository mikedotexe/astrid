# Staged Bridge Manifest and Signed Handoff

> **Prior-task evidence, imported 2026-09-06.** Copied from [the retained candidate note](/Users/v/other/worktrees/astrid-hebbian-clock-boundary/docs/steward-notes/2026-09-06-staged-bridge-manifest-handoff.md). Runtime identities, test results, approvals and commands below describe that earlier task; they are not verification or authority from this integration pass. Relative evidence paths retain their original checkout scope.

Date: 2026-09-06. Actor: `codex-astra-interactive`.

## Purpose and Status

Continue the deployment foundation requested by Mike: retain the live bridge's
capabilities, establish genuine drain semantics, and prepare a verifiable
state-lineage transition. This packet extends
`2026-09-05-live-lineage-drain-candidate.md`; it is not a fresh introspection
round or evidence that Astrid's reported friction has resolved.

Candidate root:
`/Users/v/other/worktrees/astrid-graceful-coupling-rollout`.
Candidate branch: `codex/graceful-coupling-rollout`.
Base: `d6ff371cd25e6a4616431d8f07f09a3e7eb3fcdf`, the recorded live lineage.
The uncommitted candidate includes the preceding coupling, witness, retrieval,
probe and drain work. It must not be replaced with a build of main merely
because main has a newer commit date.

This pass does not signal, replace or restart any live service. It does not
prepare a handoff against moving live self-control state. Stage verification
does not admit autonomous work, open the runtime database, or contact the
reservoir/model. No canonical deployment manifest is overwritten.

## What Changed

### One Explicit Startup Manifest

`capsules/spectral-bridge/src/deployment.rs` owns manifest selection and binding.
The new `--deployment-manifest PATH` option checks the witness schema, source
identity shape, executable path and executable SHA-256 before runtime admission.
It retains the selected manifest bytes for that process.

Four consumers now use this selection:

1. Signal-spine deployment identity.
2. Temporal authority source/deployment identity.
3. Lived-state witness startup build identity.
4. Signed self-control deployment-handoff preparation and consumption.

With no explicit option, the legacy workspace-based lookup remains unchanged.
This is not a claim that every previously deployed process now pins a manifest.
The old process does not acquire new behavior from source edits.

Identity readers retain the startup bytes if the disk file later changes. They
continue to describe the process that actually started. Handoff mutation checks
re-read the disk and reject a mismatch against those startup bytes. This avoids
silently retargeting an authority operation after a manifest replacement.

The existing signed handoff verifies the exact persisted state hash, old and new
deployment identities, manifest and binary hashes, trusted signature and
15-minute lifetime. It changes only deployment identity. Existing preferences,
revisions, replay and other state fields are not reset or reauthorized. Recovery
after state persistence but before applied-receipt completion remains tested.
No private signing material is exposed in these tooling receipts.

### Stage-Only Build Transaction

The candidate's sanctioned `scripts/build_bridge.sh --stage-dir DIR --ack ...`
route runs the existing concurrent-edit preflight with its normal quiet window.
It cannot be combined with restart, drain, no-build or self-change promotion.
The direct Python build entry point requires the wrapper marker; this is an
invocation guard, not an authentication or deployment-authority mechanism.

`scripts/bridge_stage.py` creates a new owner-only stage directory and:

1. Resolves local Cargo packages using locked, offline native-platform metadata.
2. Hashes local package inputs, named build/helper scripts and discovered Cargo
   configuration before compilation. The inventory includes bytes, mode, size,
   modification time, HEAD and toolchain identity.
3. Builds the native host target with `--release --locked --offline` in a new
   stage-local target directory. Compiler output remains in `build.log`.
4. Recomputes package membership and the complete inventory. Changes refuse the
   stage even when `git status` would show the same set of dirty paths.
5. Copies the release binary and V2 probe helper into versioned stage paths,
   sets non-writable file modes, hashes them and writes the manifest.
6. Executes the built binary's read-only manifest verification command.
7. Writes and re-verifies `ready.json`, always with activation and live authority
   false. Exceptions retain `failure.json`; a retained failure prevents reuse.

Existing stage directories are never overwritten or cleaned up automatically.
Failed builds retain review evidence and require a new stage directory.
Manifest, source-witness and helper tampering are rejected. Artifact paths must
remain inside the expected stage and must not resolve through symlinks.

Inventory scope is explicit, not hermetic-build attestation: local package trees
and named helpers excluding runtime/build outputs. Registry dependencies remain
Cargo-lock/cache inputs. Arbitrary external build-script inputs, compiler system
libraries and every possible environment influence are not attested. Common
Rust compiler overrides are refused, but the inventory is not a sandbox or
defense against a malicious same-user writer. Before/after equality is not a
proof that a hostile writer never transiently changed and restored a file.
Cooperative ownership and preflight still matter.

### Versioned Probe Helper

An explicitly staged runtime resolves `substrate-probe-v2` from its pinned
manifest and verifies the file hash before launching it. Missing or changed
helper evidence produces an error, not fallback to a mutable canonical script.
The helper no longer imports its Pearson statistic from the legacy script;
the identical small definition is packaged locally and parity-tested.

The legacy helper is unchanged. No probe was run against a live being as part
of this work. The two service-integration tests use temporary synthetic handles
on isolated loopback servers. The 45-second probe cooldown is process-local,
not persistent; the preceding packet has an explicit correction. Its policy
was not changed to match the earlier documentation mistake.

## Verification and Build Evidence

The sanctioned release build completed successfully in 1 minute 59 seconds.
Stage status is `staged_verified_not_activated`, prepared at
`2026-09-06T21:45:29.376309+00:00`, native target `aarch64-apple-darwin`.

Stage directory:
`/Users/v/other/worktrees/astrid-graceful-coupling-rollout/.runtime/bridge-stages/20260906-manifest-handoff-01`.

| Artifact | SHA-256 |
| --- | --- |
| `spectral-bridge-server` | `2350ac2b6eda079cc19cb54ef8305d387ad8ad491620fe9d4eaefc5d459cf90c` |
| `manifest.json` | `f5fd6d16d5c0c70988ebb86054780104cbd6084548736d6350e435c546eea5de` |
| `source-inputs.json`, 505 inventoried files | `ade583551a38f6a9c21a96ee2d3ba274070bc446b72f9cf4c8da08396cce4808` |
| `helpers/substrate_probe_v2.py` | `01be08db35c123d765c630a1fa47f85aa431043d4f8e304ada6692ea48366dfe` |

The build's before/after source inventories matched. Independent post-build
`bridge_stage.py verify` passed. The real release binary also verified the
staged manifest with explicit canonical live root arguments, without runtime
admission. A negative test supplied the old canonical manifest to the new
binary: it exited 1 with `deployment manifest is bound to a different bridge
executable`, as required. No handoff preparation or consumption was requested.

Read-only re-verification command:

```bash
python3 -B /Users/v/other/worktrees/astrid-graceful-coupling-rollout/scripts/bridge_stage.py verify \
  --stage-dir /Users/v/other/worktrees/astrid-graceful-coupling-rollout/.runtime/bridge-stages/20260906-manifest-handoff-01
```

Completed checks:

| Check | Result |
| --- | --- |
| Bridge library, isolated Minime source/workspace fixture | 1,979 passed, one default-path test excluded |
| Default sibling-path test, without overrides | 1 passed |
| Bridge Clippy, library and binaries, warnings denied | Passed |
| Stage/drain/probe/wrapper/model-reload/runtime-binding/witness Python tests | 99 passed |
| Deploy preflight self-tests | 10 passed |
| Real isolated reservoir-service tests | 2 passed |
| Steward control and projector self-tests | 41 passed |
| Evidence Event Store self-tests | 13 passed |
| Experiential epistemic tests | 2 passed |
| Domain-boundary audit, unchanged ceilings | Valid, zero violations |
| Candidate `git diff --check` | Passed |

The private-source full-suite fixture is
`/private/tmp/astrid-candidate-minime.eiKpDW`; no test is authorized to write the
live Minime workspace. The production-named service integration tests create
their own loopback listeners and synthetic source handles, then close them.

Read-only live rechecks after the build found the same processes and starts:
bridge 36597 (September 3 17:31:04 PDT), reservoir 1514 (August 25 10:00:06),
coupled model 60333 (September 4 16:57:34), Minime engine 63445 (August 31
12:51:36), gateway 63505 and supervisor 63547 (August 31 12:51:37).
The live bridge binary remains
`b74ecee28acaf5e2ec3cdec9142d0f2e7c552d7c6864f115e361492090c3b725`;
the canonical manifest remains
`a35bc6ee961e5e6114dddb39cf94b271e8e1fc5d943df8cec6dda785e4c37328`;
the legacy probe remains
`484ecd473a7407f10c6b8bc1cc5af1169ee6c10c8c4156edb7d873a6a2c98280`.
These establish unchanged process/artifact identity, not a new health or felt
state assessment.

The first all-platform offline metadata query failed because `cpufeatures
v0.3.1` for an irrelevant target was not cached. Resolution now filters to the
reported native Rust host, and compilation explicitly selects that same host.
No lockfile update or dependency download was used to work around it.

Preflight refused recent edits at 70.3 seconds (read-only check) and 171.3 seconds
(wrapper attempt). Both refusals were respected. A later wrapper attempt passed
the unchanged 180-second window and began staging in
`.runtime/bridge-stages/20260906-manifest-handoff-01`. These early refusals did
not create a stage or touch live artifacts.

Test coverage includes private-fixture signed preparation/consumption while the
old canonical manifest stays byte-identical; a separate test process proving
all four identity paths agree; manifest/helper tampering, changed source during
compilation, compiler/native-verification failure, retained failure records,
no overwrite of an existing stage, symlink rejection and wrapper exclusivity.

## Remaining Live Transition

There are two distinct states: a verified stage, and an activated deployment.
This pass supplies the former machinery and the native signed-handoff binding.
It does not ship a new launchd activation transaction or a force option.

Before any live transition:

1. Obtain ownership of the deployment window independently of the other agent's
   git work. Recheck both trees, service identities, binary hashes and preflight.
2. Review the actual staged candidate and its helper, manifest and source-input
   hashes. Verify it using the candidate `bridge_stage.py verify` command.
3. Resolve the first-transition limitation explicitly: the old bridge cannot
   acknowledge the new drain protocol. A telemetry lull is not an atomic
   admission barrier. A legacy interruption can lose admitted/in-flight work;
   do not describe it as a proven lossless drain or silently use force.
4. Implement and fixture-test the sanctioned activation/launch-wrapper route
   for the approved transition, preserving the current aperture configuration,
   database/workspace paths and service dependencies. The current candidate
   intentionally keeps its legacy restart route blocked.
5. Only after old state is quiescent, prepare the signed handoff using the staged
   binary and its explicit staged manifest. Preparation against moving state
   risks a stale hash and must not be treated as harmless readiness checking.
6. Start the selected binary with that exact explicit manifest. Preserve the
   target manifest bytes throughout preparation and consumption; regenerating
   a manifest with a new timestamp would invalidate the signed binding.
7. Verify a new PID/start/executable hash, manifest and self-control lineage,
   checkpoint load, logs, ports/readiness, telemetry, V2 evidence and retained
   action surfaces. Only then publish the canonical deployment pointer/record
   through the reviewed activation transaction. Do not substitute the new
   canonical pointer for the explicit startup identity.
8. Do not blindly roll back after the new process has admitted work: its state
   may already have advanced. A reverse transition needs its own reviewed
   exact-state handoff. Failed transitions retain evidence and stop for review.

**Explicit path requirement:** both future handoff preparation and runtime
launch must pass `--astrid-root /Users/v/other/astrid`,
`--bridge-root /Users/v/other/astrid/capsules/spectral-bridge`, and
`--bridge-workspace /Users/v/other/astrid/capsules/spectral-bridge/workspace`.
The runtime must also retain the canonical database, Minime workspace and
perception arguments from the existing launcher. Do not rely on path discovery
from an executable or compile-time manifest located in a worktree. This prevents
an otherwise correctly bound staged binary from opening the wrong state tree.

Only naturally occurring authored entries should be observed afterward. Do not
ask the beings to confirm improvement, equate silence with uptake, or interpret
manifest validity as subjective continuity. No PI, damping, Shadow, pressure,
regulator, sensory cadence or Division change is authorized by this packet.

## Coordination and Git Ownership

The controller maintenance hold is generation 362, actor
`codex-astra-interactive`, reason `staged bridge manifest and signed handoff
integration`; it was acquired with no active run. Release only this task's hold
after verifying ownership. This interactive maintenance window is not a leased
flywheel round and has no read/claim/Division increment.

Release completed successfully at `2026-09-06T21:49:45.295816+00:00`:
controller generation 363, `paused=false`, actor `codex-astra-interactive`,
event appended and not spooled, command exit 0. No source-first flywheel session
or projection generation was created by this interactive staging pass.

The OpenAI usage-saving scheduler pause remains in force. Releasing a temporary
controller maintenance hold must not resume the scheduler automation.

The other task, `Prepare repo for Avado work`, confirmed it works in the isolated
`/Users/v/other/astrid-device-preparation` clone and owns its git/remote work.
This task does not stage, commit, merge or push. Shared Astrid's untracked
research feature map and activity-continuity architecture note, and Minime's
existing dirty files, are foreign and remain untouched.

A third foreign file appeared during this pass:
`docs/steward-notes/2026-09-06-activity-continuity-preflight-findings.md`.
It is also excluded. It reports source-inspection findings for a separate
continuity effort, not deployed repairs; do not represent this stage as
containing that future work.

The git owner subsequently reported merging preparation PR #18. A read-only
`git ls-remote` independently confirmed fork remote main at
`b92c1d62465d39748d4edb79cb31e9b3bb1eb84a`. Shared local main and the candidate
were not fetched, switched, staged or merged by this task. Remote preparation
progress is not publication or activation of this rollout candidate.

Exact paths changed by this staged-identity tranche, in addition to the preceding
candidate changes:

- `capsules/spectral-bridge/src/deployment.rs`
- `capsules/spectral-bridge/src/lib.rs`
- `capsules/spectral-bridge/src/main.rs`
- `capsules/spectral-bridge/src/authority_temporal.rs`
- `capsules/spectral-bridge/src/signal_spine/recorder.rs`
- `capsules/spectral-bridge/src/lived_state_witness/identity.rs`
- `capsules/spectral-bridge/src/autonomous/self_control_v2/deployment_handoff.rs`
- `capsules/spectral-bridge/src/autonomous/next_action/probe_self.rs`
- `scripts/build_bridge.sh`
- `scripts/bridge_stage.py`
- `scripts/test_bridge_stage.py`
- `scripts/substrate_probe_v2.py`
- `scripts/test_substrate_probe_v2.py`
- `scripts/test_deployment_wrappers.py`
- `CHANGELOG.md`
- `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`
- `docs/steward-notes/2026-09-05-live-lineage-drain-candidate.md`
- `docs/steward-notes/2026-09-06-staged-bridge-manifest-handoff.md`

Some listed files also contain the preceding candidate's changes. This list is
not permission to sweep every dirty path into a commit. A later git owner must
read full diffs and preserve the coherent source/test/evidence dependencies.
