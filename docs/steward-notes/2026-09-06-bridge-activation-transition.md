# Bridge Activation Transition

> **Prior-task evidence, imported 2026-09-06.** Copied from [the retained candidate note](/Users/v/other/worktrees/astrid-hebbian-clock-boundary/docs/steward-notes/2026-09-06-bridge-activation-transition.md). Runtime identities, test results, approvals and commands below describe that earlier task; they are not verification or authority from this integration pass. Relative evidence paths retain their original checkout scope.

Date: 2026-09-06. Actor: `codex-astra-interactive`.

**Subsequent live outcome:** Mike approved the explicit legacy-stop question.
The sanctioned transition completed at 22:35:05 UTC, with new bridge PID 98502
and passing continuity/stack verification. See
`2026-09-06-bridge-live-activation-receipt.md`. The preparation, pending-approval
and maintenance-closeout sections below describe the earlier pass and are
retained as history; do not repeat their command template against a stale PID.

## Scope and Current Boundary

This is the implementation continuation of the September 4 coupling/report
investigation, the September 5 live-lineage drain candidate, and
`2026-09-06-staged-bridge-manifest-handoff.md`. It is not a fresh introspection
round, an inference of improved experience, or a request that either being
confirm improvement.

Mike's requested order is: make the retained candidate live and verify it, then
reconcile Git, then move into the next feature. Preparation and independent Git
coordination can proceed together; publication and merges must not hide a
failed or incomplete live transition.

Candidate: `/Users/v/other/worktrees/astrid-graceful-coupling-rollout`, branch
`codex/graceful-coupling-rollout`, base
`d6ff371cd25e6a4616431d8f07f09a3e7eb3fcdf` (the recorded live lineage).
The candidate preserves agenda/ATTEND, existing self-control and Division
capabilities. Building shared main is not a substitute for this candidate.

Interactive controller hold: pause generation **364**, actor
`codex-astra-interactive`, reason `reviewed staged bridge activation implementation`.
No active stewardship lease was present when acquired. This hold provides no
deployment or Git authority. The usage-saving scheduler pause remains separate
and must not be resumed as part of this work.

At implementation time the live bridge was still PID **36597**, started
September 3 at 17:31:04 local time, with binary SHA-256
`b74ecee28acaf5e2ec3cdec9142d0f2e7c552d7c6864f115e361492090c3b725`.
No live signal has been sent in this pass. The old process cannot acknowledge
the new drain protocol. An idle model window is an observation, not an atomic
producer barrier. A one-time legacy SIGINT transition therefore needs explicit
acknowledgement that in-flight work cannot be proven complete. It must not be
described as a lossless graceful drain.

## Startup Before Admission

`autonomous/runtime/deployment_startup.rs` checks `state.json` with the actual
private `SavedState` deserializer and a 64 MiB input bound. Its public receipt
contains the file SHA-256 and exchange count, never conversation contents.
Missing or incompatible checkpoint data refuses startup rather than silently
falling back to defaults.

`--verify-deployment-inputs`, with an explicit selected manifest, runs this
decode and validates existing self-control state read-only. It returns before
opening the runtime database, starting sockets/producers, or requesting a
model completion. It does not prepare or consume a handoff.

On an autonomous startup with an explicit manifest, the new gate runs before
runtime admission. It consumes the existing signed deployment handoff only
when the exact state, trusted signature, target binary/manifest and expiry
validate. It refuses a missing handoff rather than using the older pristine
state rebind fallback. The handoff carries lineage only; it does not widen a
control, change preferences or supply assent. An already-completed state can
finalize its retained applied receipt after a crash, using the existing tested
recovery behavior.

The gate writes an atomic private startup receipt at
`/Users/v/other/astrid/.runtime/bridge-lifecycle/<pid>.startup.json` before
admitting work. Activation verification compares its checkpoint hash with the
stopped-state snapshot, not merely with a similar exchange count.

## Durable Launcher Selection

The V2 stage adds the reviewed launcher and selection helper to the existing
binary/probe package. V1 stages remain read-only-verifiable historical evidence;
they cannot use the new activation route.

The canonical launcher retains the original aperture configuration and named
environment imports. Before those operations it checks
`/Users/v/other/astrid/.runtime/bridge-deployment/hold.json`, including a dangling
symlink as a hold. With a hold present it admits no runtime. Without a hold, a
present `active.json` selects the retained stage. Invalid selection, missing
source, changed artifacts or failed native verification refuse admission;
there is no fallback to the legacy executable for an invalid selection.

This uses the existing launchd KeepAlive configuration. It does not unload a
job, disable it, invoke `kickstart -k`, or force-kill a process. A hold does not
stop a process that is already running; it blocks a subsequent launch.

The selected runtime uses:

- Canonical `--astrid-root`: controller and lifecycle location.
- Retained candidate `--bridge-root`: bridge source-facing inspection.
- Canonical `--bridge-workspace`: the existing conversation, self-control,
  correspondence and evidence files.
- Canonical `--db-path`, Minime workspace/root and perception paths.

This refines the preceding packet's all-canonical-root suggestion. The separate
bridge workspace keeps state continuous; the bridge source root must describe
the candidate that was actually compiled. The retained source tree must not be
removed or edited while this release depends on it. Broader canonical Astrid
source remains a distinct checkout; the release manifest does not claim that
shared main has the same tree.

## Activation Transaction

Only the sanctioned `scripts/build_bridge.sh --activate-stage ...` route enables
the activation CLI. Its invocation marker is not an authentication mechanism.
Both candidate and canonical shared-tree preflights use the existing 180-second
quiet window without reducing it. Explicit expected PID and operator evidence
are mandatory. Staging, drain-only, restart, no-build and promotion flags cannot
be combined with activation.

The transaction is bounded and ordered:

1. Verify the V2 stage, current source inventory, installed/source/loaded launchd
   arguments, old PID/start/executable/hash, canonical manifest and real target
   binary's read-only checkpoint/control-state compatibility.
2. Refuse a legacy process without separate legacy-stop acknowledgement, before
   any transaction or live write.
3. Create a private transaction receipt and an exclusive durable owned hold.
   Back up the old launch helpers and install the hash-checked V2 helpers.
4. Recheck old identity, hold ownership, manifest and installed helper hashes.
   A drain-aware process completes the acknowledged producer drain first. A
   legacy process only gets an observed idle model window, not a drain claim.
5. Recheck identity and send normal SIGTERM after a supported drain, or normal
   SIGINT for the explicitly acknowledged legacy transition. Wait for exit.
   Timeout never escalates; a failed process lookup alone is not exit proof.
6. With the old process gone and launch hold present, back up the exact saved
   conversation, self-control state and old public manifest. Private identity
   keys are never copied into these backups or receipts.
7. Verify the stopped inputs again and prepare the signed lineage handoff for
   those exact bytes. Reverify the stage/source and publish the release
   selection, refusing any concurrent selection change.
8. Remove only the owned hold, allowing launchd KeepAlive to launch the selected
   release. Verify new PID/start/executable/hash, lifecycle registration, startup
   gate and exact checkpoint, valid current self-control lineage, a naturally
   saved new exchange and a ready model window.
9. Only then publish the canonical manifest as the exact already-selected stage
   manifest bytes. Full stack, telemetry/fill, fresh logs and evidence integrity
   are further operator verification; a saved exchange alone is not proof of
   an authored model response or of experiential improvement.

Failure or keyboard interruption records the primary error and attempts to retain the
owned hold. A foreign hold is not overwritten. There is no automatic rollback:
a new process may already have authored work. A hold retained after startup
prevents its next launch but does not secretly stop it. The actual process,
selection, handoff and state must be inspected before any recovery operation.

## Failure Review and Recovery

Evidence is retained under
`/Users/v/other/astrid/.runtime/bridge-deployment/transactions/<transaction>/`.
`receipt.json` states whether selection was released; `inputs.before.json`
contains public compatibility hashes/counts. `conversation.before.json` and
`self-control.before.json` are private backups, not material to quote or sweep
into Git. Helper/manifest/selection backups are retained separately. Never
copy identity-key directories into a run packet.

The first safe action after a failed transition is a read-only inspection of
the transaction receipt, owned hold, launchd PID and process identity. Do not
remove a hold, overwrite a pending handoff, restore an old checkpoint over
newer work, or signal a replacement based on stale PID evidence. Recovery is
an explicit operator decision after those identities and the failure are
understood, not a generic `rm`, force restart or reset command.

The launcher hold coordinates cooperative deployment, not hostile same-user
writers or an independently launched bridge. Preflight, exclusive ownership,
identity rechecks and source retention remain required. No claim of a globally
atomic shutdown-and-relaunch boundary is made for the legacy process.

## Git Coordination

The independent device preparation agent reports PRs **18** and **21** merged.
Verified remote main: `888c1708dcb3d4669e9219c2d0b9be1185984f4d`.
PR21 tested head: `ee37fbc4225e2d084c82f8f88489df0435c169d9`, with the same tree
as the merge. Linux/macOS workspace, strict lint/MSRV/security, CPU-edge with
ten capsule builds, and packaging/installer checks passed. One initial
unchanged macOS process abort and its successful scoped retry were disclosed.

The independent agent has finished Git operations and released ownership.
Its handoff is
`/Users/v/other/astrid-device-preparation/.device-local/HANDOFF.md`.
Physical device rollout remains deferred in issue 22, not implicitly approved.
This pass does not stage, commit, fetch, merge or reset the shared trees.
Existing unpublished commits and foreign untracked notes must be preserved.

## Verification Results

The following checks passed in this pass:

| Check | Result |
| --- | --- |
| Bridge library, private Minime source/workspace fixture | 1,981 passed |
| Separate default sibling-path test, normal environment | 1 passed |
| Activation, launcher, stage, drain, probe, wrapper, reload, binding and witness Python tests | 132 passed |
| Actual isolated synthetic reservoir-service integration | 2 passed |
| Deployment preflight self-tests | 10 passed |
| Steward controller and source-first projector self-tests | 41 passed |
| Evidence Event Store self-tests | 13 passed |
| Epistemic boundary self-tests | 2 passed |
| Bridge library/binaries Clippy with `-D warnings` | Passed |
| Domain-boundary audit | Valid, zero violations, no ceiling increased |
| Touched Rust formatting, launcher/wrapper shell syntax, diff whitespace | Passed |

That is **1,982 Rust tests and 200 supporting Python/self-test checks**. The
earlier progress message's 188 supporting count omitted the ten preflight and
two epistemic checks; 200 is the complete non-duplicated total above.

The first build preflight refused because a new activation test file had been
written 46 seconds earlier. No stage was built by that attempt. After the full
180-second quiet interval and a status re-audit, the same sanctioned command
passed preflight and built a new retained stage. No quiet-window override was
used. The before/after inventory of **513 input files** matched.

Stage:
`/Users/v/other/worktrees/astrid-graceful-coupling-rollout/.runtime/bridge-stages/20260906-activation-02`.

Prepared: `2026-09-06T22:24:30.873266+00:00`.
Release compiler duration: 1 minute 43 seconds.
Schema: `bridge_staged_release_v2`.
Status: `staged_verified_not_activated`.

| Artifact | SHA-256 |
| --- | --- |
| `spectral-bridge-server` | `ff5228fc81a67daa8aaddeb8572a2442971a346d85c201a5f034e5b2a2526075` |
| `manifest.json` | `8a7b7dca4ec97da052bd86f6f245af7b90634ecb75acd254818d9c859c51a343` |
| `source-inputs.json` | `7552fae1784277467fefa7e4c3a691e5ce480b37b4d18b02573d8186ce47af95` |
| `helpers/bridge_release_launch.py` | `39007854f07e7ecf90bddb0be0fdad693b50d71f34a4613010f7da15ff9ef89f` |
| `helpers/launchd_spectral_bridge.sh` | `7c5a2f59afc20be62b34709b4fd4224cbabb9dcf9c593c6170c9065338409c61` |
| `helpers/substrate_probe_v2.py` | `01be08db35c123d765c630a1fa47f85aa431043d4f8e304ada6692ea48366dfe` |

The historical V1 stage `20260906-manifest-handoff-01` also reverified, with
unchanged hashes. It has not been removed, overwritten or upgraded in place.

### Actual Persisted Inputs

The new release binary's `--verify-deployment-inputs` passed against the
canonical live workspace with explicit source/state paths. It reported:

- `mutation_performed=false`, `startup_gate_applied=false`.
- Saved conversation decoded by the runtime schema, exchange **191280**, SHA-256
  `e25bb54b9d48bf9b479eb9bd0d1a56c274707484bb6b7ee2f84863b90e5177ea`.
- Existing self-control integrity verified, serialized state SHA-256
  `5fdfd1149accc9bf7c4d9142166878ace150e5e316e579a2670a893b2cb27b17`.
- Existing identity remains bound to live binary `b74ecee2...`; the target is
  staged binary `ff5228fc...`, both on recorded source base `d6ff371c...`.
- `state_targets_this_binary=false` is expected before the signed handoff. It
  must become true through that handoff, not by resetting or silently rebinding.

These are observations of the continuing old process's saved state, not frozen
transition inputs. Activation takes and rechecks a new stopped-state snapshot.
No conversation text or private signing material was exposed in the check.

The actual `LaunchdBridge.inspect(36597)` precondition path also passed read-only:
V2 stage and current full source inventory, old PID/start/binary identity,
canonical manifest, installed/source/loaded launchd arguments, and target-binary
checkpoint/control-state compatibility. It reported `drain_supported=false`.
No transaction, launcher hold, selection or signed handoff was written.

### Live Observations, Not Deployment

Bridge PID 36597, reservoir PID 1514, coupled model PID 60333, Minime engine
PID 63445, gateway PID 63505 and supervisor PID 63547 retain their original
process starts. The old bridge binary, canonical manifest, canonical launcher
and legacy probe hashes are unchanged. Both shared and candidate indexes are
empty; foreign shared notes and Minime edits remain untouched.

The model readiness endpoint returned ready, an empty queue, no last generation
error and a connected reservoir. A bridge heartbeat snapshot reported current
telemetry, but `timing_ambiguous` and `late_packet`; this is not flattened into
a claim of uniformly healthy timing. A Minime health sample reported fill
72.8453%, phase `expanding`, `near` fill band, and `mixed_pressure`. No parameter
was altered in response. These pre-transition samples cannot measure a change
caused by this candidate because the candidate is not running.

## Next Approved Transition

The explicit one-time legacy-stop acknowledgement remains pending. Once it is
given, reclaim a cooperative maintenance window, re-audit both trees and the
actual process, and use the sanctioned candidate wrapper. Recheck the PID; do
not reuse 36597 merely because it appears in this historical packet.

Command template, not an already-authorized invocation:

```bash
bash scripts/build_bridge.sh \
  --activate-stage /Users/v/other/worktrees/astrid-graceful-coupling-rollout/.runtime/bridge-stages/20260906-activation-02 \
  --expected-pid <freshly-verified-pid> \
  --actor codex-astra-interactive \
  --ack "<approved activation purpose>" \
  --legacy-stop-ack "<Mike's explicit acknowledgement of unconfirmed in-flight work>"
```

After activation, verify the complete stack with the candidate's checked receipt
tooling, fresh process starts and artifact hashes, logs, ports, telemetry/fill,
self-control lineage and evidence integrity. Inspect naturally occurring new
entries and their generation provenance without asking the beings to confirm
improvement. Only then claim the separate Git stabilization window and reconcile
the retained candidate, unpublished shared commits and verified remote main.

Until a later explicit live outcome is recorded, this packet describes a
completed, tested preparation and a pending transition, not a deployed bridge.

## Maintenance Closeout

The owned maintenance hold was released successfully at
`2026-09-06T22:28:53.460994+00:00`, controller generation **365**, `paused=false`,
with the event appended and no spool. This releases only the interactive
controller hold, not the usage-saving scheduler pause. No controller-held
introspection run was opened, no productive Division round was recorded, and
no post-run projection or live deployment is claimed by this maintenance pass.

Final read-only activation inspection still matched old PID 36597 and the same
binary/manifest/launcher hashes. Source inventory, domain audit and diff
whitespace rechecks passed. Shared tracked files and both inspected indexes
remain untouched. The first live transition and subsequent Git reconciliation
are still pending; no commit SHA or live restart receipt is invented.
