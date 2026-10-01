# Narrow Engine Transition Qualification

## Scope

Codex interactive collaborator, October 1, 2026. Mike requested progress toward
a graceful restart, commit, merge and push. This packet follows the
[release/checkpoint qualification](2026-10-01-engine-release-checkpoint-qualification.md).
The originating public witness remains
`capsules/spectral-bridge/workspace/journal/dialogue_longform_1790804287.txt`,
SHA-256 `6a434c8da92117ec4df058f7e8f0059ef2ad1aca13bc08decf1cde8dcb7f8447`.
Its bytes were reverified; no authored account was rewritten.

The current old engine/gateway do not acknowledge a complete input drain.
A separate question requests approval for a bounded one-time SIGTERM transition
of the engine, gateway and supervisor only. That approval has not yet arrived.
No live process was signalled, binary installed, signed handoff prepared or
runtime preference changed during this qualification. No subjective improvement
or complete reservoir/in-flight-input continuity is claimed.

## Transition Candidate

`deploy_minime.sh --activate-stage` is a separate sanctioned path, before the
legacy broad-stop/rebuild delegation. It consumes an externally hash-bound stage
and a fresh mapped-build binding, requires explicit legacy-stop acknowledgement,
and never invokes the old broad stop/start scripts. `--check-only` performs the
checks without preparing a transition. The operator acknowledgement is supplied
by the operator; a string in a command does not independently establish approval.

The host checks both deployment preflights, paused/no-lease controller integrity,
exact stage sources, installed/source configuration, profile normalization,
dormant Division, signed-control integrity and protected service PID/start tuples.
It preserves the existing profile, PI-only restore policy, ceremonial records,
agent/model/visual/sensory processes and historical evidence.

The three launch wrappers gain an owner-held wait before opening the binary or
profile. The transition creates durable transaction-owned holds, installs only
that reviewed addition, and requests SIGTERM of exact old PID/start tuples.
KeepAlive respawns must wait in their wrappers, with no service listeners.
There is no SIGKILL, live bootout or unknown-process termination fallback.

After old writers exit, the transition requires a post-signal context file and
the old shutdown log indication, freezes the numerical startup inputs, runs the
production inspector under network/file-write denial, and rechecks signed-control
state. The old log alone is not trusted as a successful save acknowledgement.
The candidate is installed atomically and its signed handoff must bind the exact
canonical executable, binary hash and stopped state. Startup inputs and handoff
bytes are rechecked before release.

The held engine shell supplies the future PID/start identity before exec. Only
the existing dormant routing manifest is rebound to that identity and executable
hash; roots, generation, candidate-unbound status, ceremony ledger and expiry are
preserved. The supervisor starts before the gateway so its existing parent-only
authority rebind occurs first. Daughter authority is never accepted or reset.

Verification checks ports, mapped Mach-O UUIDs, process starts, fresh health,
PI-only restore, new measurement fields, the applied signed state-preservation
receipt and unchanged protected services. The bounded observation is at least
360 seconds, sampled every five seconds. The transition aborts at the existing
80% warning boundary, stale/nonfinite health, source/configuration drift or
process replacement. This is a conservative deployment-abort threshold, not a
new reservoir controller or general stability certification.

Rollback validates owned processes and rollback bytes, stops only released
replacement processes, and signs from the newest verified state to the retained
old executable. It never restores an old preference/state directory. If the new
engine never consumed its handoff, the old state remains current and the pending
candidate evidence is retained for review, not deleted. Unverified checkpoints,
unknown processes, foreign holds or failed recovery leave explicit durable debt.
There is no unattended crash-resume command: inspect the recorded phase and live
identities before any separately reviewed recovery. Holds deliberately remain
quiet after interruption rather than starting an unverified executable.

## Findings and Tests

- The initial harmless launchd fixture disproved the proposed use of `launchctl
  disable`: the loaded KeepAlive service respawned. Its failed receipt is retained
  as `2026-10-01-engine-launchd-inhibit.json`. This approach is not used.
- The replacement fixture exercises the exact hold loop with an owned sleep
  service. It prevented execution during the hold and preserved the shell PID
  through exec. Receipt: `2026-10-01-engine-launchd-hold.json`. Both temporary
  services were removed; no production service was addressed.
- Sixteen focused tests cover ordered intent receipts, early failures, rollback
  requests and failures, exact PID refusal, no forced termination, owner/corrupt
  holds, symlinks, launcher scope, atomic-write failure, stale/invalid health,
  signed-handoff bindings and newest-state rollback. These are fixtures, not a
  live restart or exhaustive crash qualification.
- The first focused run had three fixture-path failures because macOS temporary
  paths traversed `/var`'s symlink. Resolving the fixture root fixed those tests;
  production symlink refusal was not weakened. The repeat passed all sixteen.
- Existing deployment-wrapper tests passed sixteen tests. Supporting engine,
  stage, inspector, controller, evidence and projector qualification is retained
  in the preceding packet; integration reruns are recorded below when complete.
- The combined transition/support rerun passed 148 tests, including the actual
  staged inspector under OS network/write denial. Receipt:
  `2026-10-01-engine-transition-support-tests.txt`. Domain-boundary verification
  reported zero violations; Minime formatting and both whitespace checks passed.
- A real `--check-only` run passed without activation. The binding at
  `capsules/spectral-bridge/workspace/deployment_manifests/minime-runtime-binding.my2sccw0/runtime-binding.json`
  hashes to `589cff280db1640fbd74d9febd32633b9ffa989a751c141d9b5d808116a25bd8`.
  Its 180-second freshness limit still applies; recapture before any transition.

The qualified stage remains `release-03`, manifest
`0d1ab9df81bc072318554147112dc98258b393b2c796a3e1fba2fd54d57cfed8`.
Its source inventory must be reconciled again after Git integration; do not label
an old staged binary as built from a later merge commit.

## Git and Remaining Debt

Both canonical main trees were clean at the start and again before stabilization.
Remote tips were rechecked: Astrid `13dd1e8e8dee656ba55fe323017a03130f592feb`,
Minime `eb342a0c30bfa6867dff548ba14e5f0b97a1e6fa`. One interactive Git coordinator
claimed pause generation 485 with no active lease. Previously paused automations
remain paused; this is not an automation round or source-queue-current claim.

The staged-state rerun passes all 833 selected Rust tests. Four historical
Minime test transcripts retain their original blank final line:
`2026-09-30-basis-repair-tests.txt`, `2026-09-30-fill-coupled-tests.txt`,
`2026-10-01-engine-restore-tests.txt` and
`2026-10-01-engine-restore-tests-final.txt`. Default cached whitespace checking
reports only those four evidence-format findings; repeating with the explicit
`core.whitespace=-blank-at-eof` exception passes. No transcript was normalized.
Staged bytes match the reviewed working files; structured numerical receipts
remain valid JSON with their original hashes, including unsuccessful runs.

Remaining: finish integration checks and exact-path commits, reconcile with
current main, push, and requalify final source identities. Actual stopped-state,
forward/rollback handoff and live observation receipts can only be obtained
during an approved transition. The historical estimator-delay/high-excursion
findings remain explicit. Successful launch fixtures do not resolve them.
