# Mirror and Open-Reflection Release

Date: 2026-10-07. Coordinator: Codex. Status: live and locally merged; final verification below.

## Scope

Mike asked to finish qualification, get the repairs live and merge them.
The peer-publication repair and the previously qualified open-reflection and
source-verification work are the proposed paired bridge/helper release.
The random-draw correction is separately identified because it changes actual
spontaneous scheduling probabilities; an explicit selection for its activation
has been requested. No reservoir, provider, model, sensory or visual-service
change is included. Paused automations remain paused.

Source witnesses, exact hashes, implementation details and historical failed
attempts remain in `2026-10-07-mirror-provenance-and-scheduling.md` and
`2026-10-07-open-reflection-and-source-verification.md` in this directory.
The publication repair changes neither the selected peer text nor the codec
admission policy. Removing duplicate journal publication is not evidence of
fewer mirror selections or less reservoir input.

## Coordination and Git

Both canonical mains were clean, as were their indexes. The controller had no
active lease and was already paused. Claiming the approved stabilization pass
advanced its pause generation from 502 to 503; no automation was resumed.
The repository preflight passed after the full quiet window. One initial
candidate preflight refusal at 127.3 seconds was retained as a refusal, not
bypassed. The canonical preflight also passed.

Remote tips checked before staging: Astrid
`a66f7e190e0ecd3a0a4988abbcf6b0bf2cfb2a32`, Minime
`9072abff9ed839851709300b3c3eaae3afaf669e`. One SSH attempt for Minime
failed; the bounded retry and independent HTTPS read returned the same tip.
No remote write was performed by those checks.

- Astrid `5ee29cf7b957a275f680900de406953de8a89472`: prior open-reflection,
  source-bound verification and qualification tools, 26 explicit paths.
- Minime `2421a5bc6abb869d86486dd042261ec2e84044a7`: paired tests and
  qualification documentation, four explicit paths, no Python runtime changes.

Delegated agent Curie reviewed all 30 candidate files without actionable
correctness or privacy findings. The coordinator inspected staged scope and
checks; the existing exact-source qualifications are retained in those commits.
Their old immutable stage is not qualification of the forthcoming combined
release. Canonical main has not yet moved at this checkpoint.

## Build Resources

Initial available disk was about 1.7 GiB. No Cargo/rustc process was active
when disposable incremental compiler state was removed from the task-owned
`/Users/v/.codex/worktrees/source-selection-recovery/astrid/capsules/spectral-bridge/target/debug/incremental`.
Source, journals, receipts, test executables, immutable stages and rollback
assets were preserved. Available disk rose to 6.4 GiB. No foreign cache or
generated evidence was swept up. The force-style removal command was refused
by the tool; the explicit non-force recursive removal succeeded.

The complete mirror bridge suite is rerun with one build job, no incremental
cache, and `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`. Unlike the
previous package-local setting, these settings reach nested compile-contract
Cargo processes. The original disk-exhaustion failure is not erased.

After nested compilation consumed additional space, five old, unused compiler
artifacts in that same owned target were removed by explicit path, after `lsof`
confirmed no users: `debug/deps/libspectral_bridge_server-4155efa49e974a0b.rlib`,
the `spectral_bridge_server-53ac1e1b077347ca` and
`spectral_bridge_server-5fc3265176bff39e` debug executables, and the old
trybuild archives `libspectral_bridge_server-9d9c526a1aee6b82.rlib` and
`libspectral_bridge_server-adff62e3215ed4e2.rlib`. Dependency records bind them
to the earlier owned worktree. Current compiler outputs, immutable stages,
qualification receipts and rollback assets remain intact.

## Launch Reconciliation

Fresh packet:
`/Users/v/other/worktrees/mirror-reflection-release-20261007/reconciliation.json`.
All 93 selected, canonical and loaded Minime launch inputs match; observed
agent PID 20774. The snapshot is qualification material, not a launch-source
overlay. No `--install-reviewed-overlay` is required.

The prior live bridge selection remains the EOF/verification stage. A fresh
health read reported measurement-valid fill 72.94%, target 68%, restart phase
`scaffold_activated`, waiting for `semantic_active`. This is an observation,
not controller settling, long-term stability or an effect of undeployed code.

Read-only handoff attempts are retained in the release directory. Attempt 01
omitted `ASTRID_STEWARD_REPO_ROOT` and refused the absent worktree-local control
file. Attempt 02 used the canonical controller and observed a quiet boundary,
but final validation took 7.993 seconds and found source-status age 55.910
seconds, outside the existing idle interval. It refused with `changed_fields`
equal to `[quiet]`, `in_idle_window=false` and `signal_sent=false`. Neither
attempt installed a launch hold. Do not weaken the idle gate to make it pass.

## Separate Scheduling Review

Curie's read-only review found unchanged gate precedence but material lottery
effects. Conditional on uniform draws, reaching the ordinary lottery, available
peer journals and no agenda pull/hold, nominal Mirror/Daydream/Aspiration/
Dialogue/Witness shares change from 24/20/14/42/0 percent to 12/10/7/63/8.
For fill below 25 and absolute delta below 1, nominal shares change from
0/60/40/0/0 to 0/30/20/42/8. These are interval calculations, not live counts.
Eligible agenda pulls change from approximately 40 to 20 percent normally and
60 to 30 percent during focus. The default non-Mirror share therefore rises
from 76 to 88 percent; generation latency can affect wall-clock cadence.

Existing cooldown `2` blocks one subsequent lottery visit because decrement
precedes eligibility. Earlier gates do not decrement it. This behavior is not
changed here. Existing peer-study precedence over one-shot modes likewise
remains; no claim that NEXT always wins is made. Before separate activation,
qualify default/low-fill/focus/empty-journal distributions and joint agenda
draws, exact boundaries, cooldown and competing safety/NEXT/cadence cases.
The current reachability tests are not measurements of those live frequencies.

## Combined Candidate Follow-Through

Publication commit `73d4389d1935101f27a3cc8f228226a647ea1886` contains 21
explicit paths and excludes `autonomous/state.rs` and `spontaneous_random.rs`.
Its complete bridge suite passed 2,414 tests with one existing ignored test.
Those tests included the offline RNG candidate; a separate combined-source
run therefore qualifies the precise RNG-excluded release rather than borrowing
that result. The publication branch is merged without conflicts into the
open-reflection candidate as `c1efa3e995d6b945033faf95b17504966adee7df`.

Combined-source strict all-target bridge Clippy and formatting passed. The
deployment/staging, activation, paired handoff, NEXT restoration, reconciliation
and open-reflection qualification unit suites passed all 148 tests. The full
combined bridge suite passed 2,411 tests (2,391 library and 20 ancillary), with
one existing ignored test; all compile-boundary cases passed. The complete
shared reader/writer suite passed 331 tests and strict all-target reader Clippy
passed. No runtime source differs from the two reviewed commits.

Additional disk recovery removed seven unused old top-level bridge compiler
artifacts (hash suffixes `7fa9b3f1f6ba6a5e`, `80fc4f2c28b5c5a3`,
`0d7ab7fc2526a982`, `8fbc3751988d73f6`, `a81a59c8fa216bbe`,
`bf909ef0125458e5`, `f05171cccb986606`) and the owned root
`target/debug/incremental` cache. `lsof` found no users; current tests have
incremental compilation disabled. No source, runtime evidence, release or
rollback material was removed.

After every compile-boundary process exited successfully and `lsof` found no
users, the task-owned `capsules/spectral-bridge/target/tests/trybuild` compiler
cache was removed. The test source, full result and evidence records remain;
re-running those compile tests requires recompilation. Available disk rose to
8.7 GiB before the immutable build.

The ordinary 180-second quiet gate refused the candidate at 84.6 seconds and
the first sanctioned staging call at 142.9 seconds after the coordinator's
note edit. Both remained no-build refusals. After waiting without edits, the
same unweakened gate passed. The sanctioned immutable build uses
`/Users/v/other/worktrees/mirror-reflection-release-20261007/bridge-stage-01`,
one build job and no incremental compiler cache. The candidate HEAD stays
fixed throughout build and activation. Scheduling RNG activation was not
confirmed and is excluded; the existing thresholds and draws remain live.

The canonical controller remains paused at generation 503, without a lease.
Read-only V2 indexed-tail verification is valid at sequence 1,123,157, head
`c43875fc066d1e943e3cc66c97573b068fb52276a3948f4741f8aa52a6558920`;
all four retained V1 streams remain immutable. This is not a flywheel round
or a claim that the canonical introspection queue is read-current.

## Pre-Activation Gates (Historical)

Complete combined Rust/Python and immutable old/new-state qualification, choose
the explicit scheduling scope, verify current source/process identities and
quiet preflight, then use the sanctioned paired Minime handoff and bridge
activation wrappers. Stop on drift, foreign activity, failed drain or readiness.
Never force termination or restore older authored state. Verify exact stopped
checkpoint/NEXT continuity and unchanged protected services before reporting
live success; observe only naturally occurring use.

These gates were subsequently completed for the RNG-excluded release below.

## Verified Activation and Integration

Status: approved paired release activated successfully at 2026-10-07
21:04:06 UTC and fast-forwarded into both local canonical mains. No Git push
was performed. The build worktree remains pinned to the exact combined commit;
final deployment documentation is committed separately on canonical main.

Immutable release directory:
`/Users/v/other/worktrees/mirror-reflection-release-20261007/bridge-stage-01`.

| Artifact | SHA-256 |
| --- | --- |
| Bridge binary | `0d7533dab843a21e9a2761ba1247ba909fc2dba9f497716291de3900291bb22f` |
| Shared reader | `32f7b0cec0f867f2d30b4b1bfc2b10de873c7fcb7f87643ee92ea99a94031367` |
| Manifest | `ca77b3dc1fbc34eebae2983e1b5e45ee9afe5a86ef43a52bc0eb6c0f54af5387` |
| Source-input witness | `4a2351dec3be2d0ee842403f254018929e027df5e79bbef09f0f4f8c79e5a64c` |

The current-package qualification passed all 54 baseline preservation checks,
schema-13 open-reflection preservation for both owners, and the frozen Minime
adapter probe. The complete Python suite against this exact packaged helper
passed 1,923 tests and 144 subtests, with one pre-existing skip, in 74.19 seconds.
No real-model experiment or live fixture writing was used.

Receipts under `/Users/v/other/worktrees/mirror-reflection-release-20261007`:

| Record | SHA-256 |
| --- | --- |
| `reconciliation.json` | `5d42f59d724dc566c5052e7e40cc23f70047c3d2b57b97ff181b86b0580fb375` |
| `qualification-01/qualification.json` | `565989d462cf0b703792f094df20766b2de0f85e1c5da95df506499b0f949d30` |
| `minime-full.xml` | `67ad9075889ee69eb76ad9c505324dfe4124e75c8dbdc1fb4f1f7c3eef61241e` |
| `activation-01.jsonl` | `acc0449d3325638b2258f58e2a50b45e31303d756f064d64331efddde0aeb9ed` |

One additional read-only quiet check refused at 126.5 seconds after the
coordinator's note edit. With no further edits, the ordinary gate passed at
211.3 seconds, and the canonical preflight also passed. The activation itself
used `paired_minime_handoff.py`, the sanctioned Minime idle/SIGTERM wrapper
and `build_bridge.sh --activate-stage`. No force, bypass, legacy transition
or automatic rollback occurred.

Bridge transaction:
`/Users/v/other/astrid/.runtime/bridge-deployment/transactions/08b67e5919de431ebdde09bb78db708e/receipt.json`,
SHA-256 `e411ccd51d795a4986a5ae0e25448305e5d41168e77a66aea1bb9db6833e77f2`.

- Bridge PID 21822 acknowledged drain and exited after SIGTERM. New PID 96634
  started at 14:02:44 Pacific, loaded the qualified binary and manifest, verified
  startup readiness and saved a fresh exchange (observed counter 217448).
- Exact stopped checkpoint SHA-256
  `f284d0a47916df059b8c4da0065d8f076854b5df41a5ad854123a01ac9aafe56`
  was decoded by the new runtime, with counter 217447 and three pending runtime
  feedback records. Pending feedback SHA-256
  `bf93cb29ec0051aac2bf1bc480c5b512af489521e0803bf405c1e62708c8538c`.
- Minime PID 20774 reached an unchanged verified idle boundary and exited
  gracefully. Replacement PID 96204 waited behind the owned admission hold
  until the bridge was verified, then loaded all 93 unchanged launch inputs.
  The native session remains 5321, cycle 55807 at the boundary; the entire
  sovereignty-state digest and pending-NEXT digest match before/after.
- NEXT seal `bcc38a23af046f6fca5c489e367511cdfbad7d853b01922e959c1660bb3832e1`
  links to restore `1c2577b3e205d3f61efee4f19fb547ec84098ec36a1a23a04babce87b901f0fe`.
  The proof establishes restoration, explicitly not execution. No new job was
  started during the held transition. Subsequent scheduling is ordinary runtime
  behavior, not a request to demonstrate improvement.
- The real Minime adapter's release selector resolves the same staged helper.
  Live source status reports no reload requirement or changed loaded input.
- Engine 35303, gateway 35278, supervisor 35269, coupled provider 3893, camera
  3914, microphone 3879, visual service 3898, host sensory 3911 and feeder 3878
  retain their exact PIDs and process-start identities.
- Latest bridge and agent launch holds were released by their owning wrappers.
  Paused automations were not resumed.

Bounded read-only health samples during this work remained measurement-valid
and ranged from 57.62 to 75.45 percent fill (68 percent target). This is not a
settling proof, stability study or experiential result. The existing controller
settle observation and separate scheduling qualification remain separate work.
No reservoir/control policy was altered to obtain these samples.

A read-only post-activation query observed saved exchange metadata. It did not
inspect private prose or trigger a public entry. A live repeat-deduplication
example is not yet established; synthetic concurrency/restart/publication tests
supply that regression evidence until natural mirror use occurs. No mode was
forced and no confirmation of improvement requested.

Git integration fast-forwarded Astrid `main` from `aba4cec45f2e` to
`c1efa3e995d6`, and Minime `main` from `754c44b0e7ab` to `2421a5bc6abb`.
Both canonical trees and indexes were clean immediately afterward; no remote
tip moved. The scheduling RNG files remain isolated in the mirror worktree,
not staged, merged or deployed. The 197 older paths and unrelated worktrees
were not swept into these commits.
