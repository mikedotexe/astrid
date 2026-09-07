# Hebbian Learning Clock Boundary Repair

> **Prior-task evidence, imported 2026-09-06.** Copied from [the retained candidate note](/Users/v/other/worktrees/astrid-hebbian-clock-boundary/docs/steward-notes/2026-09-06-hebbian-clock-boundary-repair.md). Runtime identities, test results, approvals and commands below describe that earlier task; they are not verification or authority from this integration pass. Relative evidence paths retain their original checkout scope.

Date: 2026-09-06. Actor: `codex-astra-interactive`.
Status: isolated implementation; not deployed.

## Request and Ownership

Mike asked to fix the learning-queue clock mismatch discovered during the
preceding approved bridge activation. This is interactive implementation,
not a resumed introspection automation or a new felt-resolution claim.

Repair checkout:
`/Users/v/other/worktrees/astrid-hebbian-clock-boundary`, branch
`codex/hebbian-clock-boundary`.

It starts at `d6ff371cd25e6a4616431d8f07f09a3e7eb3fcdf` and carries a source copy
of the tested, deployed candidate's uncommitted implementation. The copy
excludes `.git`, `.runtime`, runtime workspace, targets, virtual environments
and node_modules; its test target is a separate APFS copy-on-write clone.
Inherited dirty files are prior candidate work, not all changes from this fix.
No files in the selected live source checkout were edited.

Controller maintenance hold: generation **368**, acquired with no active
stewardship lease, actor `codex-astra-interactive`. The usage-saving scheduler
pause remains separate and unchanged. No staging, commit, push or merge is
part of this repair. The branch/worktree creation does not resolve the earlier
Git reconciliation debt.

## Reproduction

The numerical observations in `2026-09-06-bridge-live-activation-receipt.md`
include:

- Persisted `last_hebbian_consumed_telemetry_t_ms`: **1075024066**.
- Current producer telemetry: **529076440**, row **14001704** of the canonical
  bridge database at 22:49:29 UTC.
- The old checkpoint already held pending exchanges **189539, 189541, 189546,
  190494**, with the same watermark.

The old implementation returns `None` for every incoming producer tick less
than or equal to the saved watermark. A failing regression at
`autonomous::runtime::state::tests::fresh_learning_baseline_is_not_blocked_by_previous_process_watermark`
set the captured watermark, armed a fresh baseline at 529076440, then tried
529078800. It failed its expected-ready assertion: **0 passed, 1 failed**.
No private conversation or journal text was used in that test.

The failed regression has been carried into the new queue's test module and
extended with the complete four-item numerical backlog. Old pending outcomes
must never become positive learning evidence merely because the watermark was
cleared. This repair is about valid pairing boundaries, not making counters
look busy.

## Clock Contract

The existing Minime telemetry type documents `t_ms` as time since engine
start, and the inspected producer source emits `start.elapsed().as_millis()`.
The wire does not give this learning path a durable producer boot identity.
Connection identity is not producer identity either: a reconnect can occur
without a producer restart.

`learning_clock.rs` therefore implements **bridge-local observation
continuity**, without changing the protocol:

1. Each telemetry connection gets a local random 128-bit scope, using the
   existing runtime nonce mechanism. It is bookkeeping, not authority.
2. The scope and monotonic handler-arrival timestamp are bound to the exact
   decoded packet under the same shared-state lock as the latest telemetry.
   Processing delay is included in sample age, not hidden by a later stamp.
3. Disconnect or a new connection prevents use of the old latest packet.
   A new valid packet is required before this path has a baseline again.
4. A regressing tick opens a new uncertain interval. That packet is not a
   learning baseline. A subsequent advancing packet establishes a new local
   window. An isolated out-of-order packet is not labeled a proven reboot.
5. Duplicate ticks cannot refresh sample age or be consumed a second time.
   A wire/schema rejection does not manufacture an observation.

This is local bookkeeping for the learning path only. Telemetry admission,
fill computation, controller regulation, sensory transport and other
consumers of the original packet are unchanged.

## Queue and Evidence Contract

`autonomous/learning_outcomes.rs` owns the pairing logic extracted from the
large conversation-state module. Runtime wiring is in
`autonomous/runtime/learning_feedback.rs`.

- Retain the existing maximum of **four** pending outcomes, FIFO order and
  at most one consumed outcome per advancing telemetry tick.
- Require both producer tick and monotonic arrival to be newer than the
  baseline, within the same local scope.
- Require an outcome sample no older than **30 seconds** at consumption.
- Bound the entire baseline-to-outcome window to **300 seconds**, including
  generation time. Expiry can be recorded even when no fresh telemetry is
  available. Future monotonic timestamps are also ineligible.
- These two age bounds are explicit new pairing policy for rollout review,
  not empirically established causal horizons or new regulator settings.
- Do not serialize `Instant` or reuse an observation scope after a bridge
  restart. Keep the old checkpoint field names/shape readable. Restored
  pending outcomes remain available for an explicit retirement disposition,
  but cannot teach across an unproved continuity boundary.
- Reset the consumption watermark only in a new local scope, after retiring
  incompatible pending baselines. Preserve existing scoring math, decay,
  learning-rate scale and explicit codec weights.

Reasons are recorded as `restored_without_observation_clock`,
`observation_continuity_changed`, `outcome_window_expired`, `fifo_capacity`,
`baseline_observation_unavailable`, or `invalid_baseline`.

Retirements use the existing SQLite bridge-message store, topic
`consciousness.v1.hebbian_outcome_retirement`, schema
`hebbian_outcome_retirement_v1`. They are explicitly bridge-authored diagnostic
evidence, not a being Action, causal finding or control grant. Payloads contain
exchange IDs, reason and numeric clocks; they contain no text or codec vectors.
One batch records total count and up to 16 detailed items with a truncation
flag, keeping oversized historical-input evidence bounded.

The runtime plans against a clone of the queue, writes the retirement batch,
and adopts that plan only if the evidence write succeeds. A failed write
leaves the old queue and consumption watermark unchanged for retry and does
not teach from the rejected plan. Unchanged ordinary score decay still runs.
Crash/restart may repeat a recorded retirement if the newer queue checkpoint
was not saved; this is append-only evidence, not an exactly-once claim.

## Verification Scope

Tests cover the captured obstruction and backlog, same-window FIFO behavior,
duplicate and out-of-order packets, reconnect with or without a lower clock,
bridge restart/serialization, absent/stale/future samples, expiry, invalid
baselines and capacity retirement. Production `handle_telemetry_message_at`
is exercised with synthetic packets and a private SQLite database.

A real read-only SQLite connection reproduces an evidence-insert failure;
the queue/watermark remain intact, then a writable retry records retirement.
An additional runtime test compares the resulting sidecar exactly against the
unchanged scoring implementation and retains an explicit weight and nondefault
learning-rate scale. No model completion or live experiment is needed.

Final command results are recorded in the closeout below. The first full
private-environment suite passed **1995** tests before the final scoring
preservation test and handler-arrival refinement were added; that is not being
substituted for verification of the final source.

## Separate Rollout Concern

Inspection of the unchanged `autonomous/hebbian.rs` found
`COMFORT_FILL_CENTER: f32 = 50.0`. Its reward uses distance from that center.
The repository guidance names **68%** as the current stable-core hold shelf.
These are distinct code paths and the source difference is not proof of harm,
but resuming this learning path can make the older objective consequential.

The queue fix deliberately does not change that objective, its learning gain,
controller parameters, explicit weights or the being's chosen learning-rate
scale. Before deployment, review why the sidecar has a different objective and
whether it should remain independent or consume an authoritative current
target. A hard-coded replacement of 50 with 68 would bypass that review and
could be wrong under future target changes.

## Deployment and Git Boundary

The selected live bridge remains PID **98502**, binary SHA-256
`ff5228fc81a67daa8aaddeb8572a2442971a346d85c201a5f034e5b2a2526075`, rooted in
`/Users/v/other/worktrees/astrid-graceful-coupling-rollout` for bridge source.
This fix is not in that binary or source tree. No live watermark, pending
queue, score, state file, PID or regulatory parameter was changed.

The next rollout must resolve the scoring-objective review, approve the
pairing-policy scope, and use the sanctioned stage/build/activation wrapper
from the repair candidate. The currently running bridge can acknowledge the
new drain protocol, so do not reuse the historical legacy-stop command or PID
without fresh identity checks. Do not deploy by overwriting the selected
source directory. Main reconciliation still requires a separate integration
checkout preserving the full live-lineage capabilities and foreign work.

## Closeout

Final-source verification completed:

| Check | Result |
| --- | --- |
| Rust library suite, isolated Minime environment, serial execution | 1996 passed; default-path test deliberately filtered |
| Default-path test, separate invocation without environment overrides | 1 passed |
| Total Rust library tests | **1997 passed, zero failures** |
| Clippy, library and binaries, `-D warnings` | Passed |
| `scripts/domain_boundary_audit.py verify` | Valid, zero violations, counter audit consistent |
| Rustfmt check on all six new modules | Passed |
| `git diff --check` | Passed |

No architecture limit or audit baseline was raised. Repository-wide
`cargo fmt --all --check` still encounters inherited formatting drift,
including `crates/astrid-minime-protocol/tests/wire_contract.rs`; unrelated
formatting was not swept into this repair. The new-module check is not a claim
that the entire inherited working tree is formatting-clean.

A read-only stage verification against the selected deployed checkout passed:
the binary and manifest hashes are unchanged, and its 513 source-input files
still hash to
`7552fae1784277467fefa7e4c3a691e5ce480b37b4d18b02573d8186ce47af95`.
The observed bridge, reservoir, model and Minime PIDs/start times remained
unchanged. No live learning reset, restart, model completion or deployment was
performed for this repair.

The maintenance hold was released at **2026-09-06T23:28:55.211455+00:00**,
controller generation **369**, actor `codex-astra-interactive`, `paused=false`.
The controller event was appended with no spool. This releases the interactive
maintenance hold only; the usage-saving automation scheduler remains paused.
No introspection read receipt or productive Division round was recorded.

### Repair-Owned Integration Inventory

Relative to the copied deployment candidate, the code delta is limited to:

```text
capsules/spectral-bridge/src/learning_clock.rs
capsules/spectral-bridge/src/lib.rs
capsules/spectral-bridge/src/autonomous/learning_outcomes.rs
capsules/spectral-bridge/src/autonomous/learning_outcomes/tests.rs
capsules/spectral-bridge/src/autonomous/runtime/learning_feedback.rs
capsules/spectral-bridge/src/autonomous/runtime/learning_feedback_tests.rs
capsules/spectral-bridge/src/autonomous/runtime.rs
capsules/spectral-bridge/src/autonomous/state.rs
capsules/spectral-bridge/src/autonomous/runtime/state_persistence.rs
capsules/spectral-bridge/src/autonomous/runtime/orchestration.rs
capsules/spectral-bridge/src/autonomous/runtime/tests.rs
capsules/spectral-bridge/src/ws/bridge_state.rs
capsules/spectral-bridge/src/ws/runtime.rs
capsules/spectral-bridge/src/ws/telemetry_port.rs
capsules/spectral-bridge/src/ws/learning_clock_tests.rs
```

Documentation additions are this packet and the new repair entries in
`CHANGELOG.md` and `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`.
These paths are an integration inventory, not permission to stage their whole
diff against Git HEAD: several carry inherited candidate edits. Nothing was
staged or committed. Scoring-objective review, explicit pairing-policy review,
graceful deployment verification and coherent Git integration remain distinct
next steps.
