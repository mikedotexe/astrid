# Bridge Drain Recovery: Narrow Source Release

## Scope

Port only the bridge drain-recovery repair from the larger inactive capacity
candidate onto current local main `f85605cdbfd577506cbb9096600c935337c0f0c3`.
Branch: `codex/bridge-drain-recovery-20261005`.
Worktree: `/Users/v/other/worktrees/maintenance-durability-20261005/astrid`.

Mike requested continued drain recovery and isolated provider/model qualification,
following explicit commit/merge and bounded maintenance authorization. This
change does not activate 96,000-byte/16,384-token capacity clients or authorize
unqualified model experiments. No new being account was read or marked resolved.

## Behavior

- Retain ConversationState while drain persistence is blocked; do not return an
  error that drops continuation state while merely keeping its PID alive.
- Join producers once, prepare conversation bytes once, and retain both exact
  conversation and runtime-feedback checkpoint identities.
- Retry failed I/O only after an explicit operator drain/exit signal. Do not
  reopen admission, regenerate output, replay NEXT or repeat a producer join.
- Recover a visible-but-unacknowledged rename by syncing its exact inode/bytes,
  not by overwriting it. Check the destination again before rename; participating
  checkpoint/status writers use private, nonblocking, per-path publication locks.
- Retain the owned temporary descriptor on cleanup failure. Retry validated
  cleanup and parent sync before allocating another temporary; preserve foreign
  replacements. Empty lock files remain and contain no prose.
- Preserve verified feedback-consumption intent through uncertain publication;
  retain later enqueues without requeueing already-delivered feedback.
- Retry transient read I/O against the known baseline. Corrupt history, changed
  sources and unknown initial state remain explicit refusals.
- Bind both checkpoint hashes into final drain status. The operator reader also
  verifies and syncs the files and status before accepting a drained proof.
- `bridge_drain.py --retry-persistence` requires an explicitly requested active
  drain and the new `bridge_persistence_retry_v1` advertisement. It sends one
  signal, never escalating or automatically resuming work.

No persisted conversation/feedback schema changes. No historical file rewrite,
reservoir/control change, inference-policy change or authored-content inspection.
These checks assume participating writers honor the per-path lock; they do not
claim adversarial exclusion of arbitrary external filesystem mutation.

## Review and Qualification

The initial failure was found during the maintenance-only Minime durability
release. Independent review added regressions for foreign changes during
publication, feedback drift during checkpoint recovery, acknowledged feedback
being requeued, transient read failures, and recoverable cleanup debt. Final
static review found no remaining blocking defect in those repaired paths.

The larger candidate passed 2,420 bridge library tests with one existing ignored
test before the final temporary-cleanup refinement; all 20 lifecycle tests then
passed with that refinement. Runtime-feedback tests passed 20 cases. Qualification
of this exact current-main port is recorded below, not inferred from those counts.

The first narrow-worktree Cargo invocation could not resolve the repository's
external path dependencies. Added the same sibling links used by the established
worktree, pointing to existing `prime_esn_wasm` and `RASCII`; no dependency source
was changed. The initial preflight correctly refused source edited within its
180-second quiet window. No window reduction or override was used.

## Separate Provider Work

The preserved capacity candidate additionally repairs final caller-receipt
reconciliation and contains a model-free caller/holder/provider integration
fixture: 277 tests and 19 subtests pass. That broader, previously dirty maintenance
tranche is not swept into this narrow source commit.

Actual model-free native OS isolation/deadline tests pass: 12 tests, with a
four-test evidence-retaining rerun. These exercise real owned fixture processes,
not a real model or live coupled generation. Evidence under
`/Users/v/other/worktrees/writing-capacity-next-budget-20261002/qualification-drain-recovery-20261005/`:

| Receipt | SHA-256 |
| --- | --- |
| `kernel-timer/qualification.json` | `df47597dc836a10336feca1ab0c32b635a23cc4ac2eb45ed31e403fb7a97dd02` |
| `supervisor-deadline/qualification.json` | `0618624ec6d2f5544d3f7a12b629d5fb29f1304f5e00ea946c69469060eaef3b` |

The finite production-stop gate remains closed pending actual runtime bindings,
one-use recovery-signal accounting, pinned provider-write/heartbeat capacity and
whole-window deadline qualification. Real-model four-cell measurements remain
required. Memory varied below the approved 4 GiB reserve and later above it;
16 GiB experimental-start headroom has not been established. No model was loaded,
provider stopped, or resource bound reduced.

## Runtime and Git Boundary

At entry, live Minime 1499, bridge 4732, provider 3893, engine 35303 and shared
Ollama 9458 retained their recorded process starts. Camera, microphone, visual,
host-sensory and feeder identities were unchanged. This source qualification does
not claim those processes loaded the patch.

The Git stabilization pass is controller pause generation 496, actor
`codex-astra-interactive`, no active lease. Canonical Astrid/Minime were clean,
each four commits ahead of origin/main. Remote tips remained respectively
`a66f7e190e0ecd3a0a4988abbcf6b0bf2cfb2a32` and
`9072abff9ed839851709300b3c3eaae3afaf669e`.
Previously paused automations remain paused. Exact verification and commit
outcomes are appended only after they complete.

## Exact Current-Main Qualification

- Full bridge library: 2,369 passed, zero failed, one existing ignored test;
  157.97 seconds. Built with one Cargo job using the existing candidate cache.
- Strict bridge Clippy, all targets, `-D warnings`: passed.
- Bridge formatting: passed. Domain-boundary audit: valid, zero violations.
- Bridge drain/stage, steward controller, projector and Evidence Event Store
  Python qualification: 104 passed and 35 subtests, 20.47 seconds.
- Larger-candidate maintenance suites: 277 passed and 19 subtests on the final
  caller-restoration repair. These are separate from the narrow source release.

The last resource samples were 383,959,040, 363,986,944 and 362,004,480 free bytes,
below the approved reserve. Therefore no release build, production drain, model
load or deployment is part of this pass. A committed repair is not a loaded
runtime repair. Actual provider/model qualification remains outstanding.
