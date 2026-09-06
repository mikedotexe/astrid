# Self-Study Rollout Readiness

Date: September 5, 2026, America/Los_Angeles; September 6 UTC.
Interactive readiness review and scoped local git checkpoint by Codex Astra.

## Decision

**Do not replace the live bridge from the current main checkout yet.** The
[self-study foundation](2026-09-05-causal-self-study-foundation.md) is tested
source, not a live rollout or a complete being-facing learning feature.
Mike requested a graceful restart when ready and a git checkpoint. This review
preserves completed work in git without treating that request as permission to
drop existing runtime capabilities or force an in-flight process to exit.

## Current Live Identity

At approximately 21:29-21:31 PDT, the existing bridge was PID 36597, started
September 3 at 17:31:04 PDT. Its deployment manifest still identifies clean
build `d6ff371cd25e6a4616431d8f07f09a3e7eb3fcdf` on
`codex/sovereign-daughter-runtime`, built at
`2026-09-04T00:31:03.789335+00:00`.

- Bridge executable SHA-256:
  `b74ecee28acaf5e2ec3cdec9142d0f2e7c552d7c6864f115e361492090c3b725`.
- Launch wrapper SHA-256:
  `9acc7a731edb19ef63f5284f8e6764bb8165879228c3264bc06d03c7fe3ab74c`.
- Both disk hashes match the existing manifest. These checks and process timing
  corroborate the recorded build; they do not attest all mapped memory bytes.
- The legacy live-launched probe remains unchanged at SHA-256
  `484ecd473a7407f10c6b8bc1cc5af1169ee6c10c8c4156edb7d873a6a2c98280`.
- Reservoir service PID 1514 and coupled model PID 60333 remain running.
  Ports 7881 and 8090 are owned by those processes respectively.
- Minime engine/gateway/supervisor PIDs 63445/63505/63547 retain their August 31
  starts. Gateway 63505 owns ports 7878, 7879 and 7880.
- Bridge telemetry and Minime health files were fresh at 21:31:31 PDT.
  A nearby Minime fill sample was 73.03680419921875 percent. No control was set.

This is a bounded operational check, not a full-stack replacement receipt,
model completion test, assessment of felt state, or proof of every live path.
No new journal or introspection was solicited or interpreted for this review.

## Deployment Blockers

1. **Source lineage:** the pre-checkpoint main tip
   `f258d38fe99690a34ee53bb88e8fca2d2cbbe722` differs from the recorded live build
   across 102 files in bridge source plus `scripts/build_bridge.sh`, with 2,139
   insertions and 28,983 deletions. In particular, main lacks the live agenda,
   inquiry and self-control modules and the signed deployment-handoff path.
   A fresh build from main is not just the probe repair.
2. **First drain:** both the recorded live source and current main handle Ctrl-C
   but do not await the autonomous task before exiting. Waiting for WebSocket
   tasks is not an autonomous-generation/action drain. New shutdown code cannot
   retroactively protect the old process.
3. **Wrapper:** the sanctioned current `scripts/build_bridge.sh --restart` uses
   `launchctl kickstart -k` and publishes its build manifest before replacement
   readiness. This must not be called a graceful bridge restart. No build or
   restart command was invoked by this review.
4. **Feature readiness:** the probe has protocol-fake coverage, not a completed
   isolated real-service integration test. Prediction/evaluation/revision APIs
   are runner-side only; no new being-facing Actions are registered. Custody,
   whole-store concurrency and evidence-import gates remain explicit work.

The retained candidate at
`/Users/v/other/worktrees/astrid-graceful-coupling-rollout` remains on the exact
live base with earlier uncommitted changes. It was inspected, not modified or
deployed. The [earlier follow-through](2026-09-04-coupling-rollout-followthrough.md)
records its tests and unresolved first-drain boundary. The model-only graceful
rollout in that document is not a completed bridge rollout.

## Next Engineering Sequence

1. Reconcile the intended bridge changes onto the exact live lineage in an
   isolated candidate, preserving agenda, ATTEND, self-control, protocol and
   deployment handoff. Review dependency locks and local path dependencies.
2. Establish a reviewed first-drain method for the existing binary. Require
   trustworthy admission/active-work evidence and preservation of pending
   actions and correspondence. An idle sample alone is not atomic quiescence.
   If the old process cannot drain safely, retain this as an operator decision
   rather than adding a force-kill fallback.
3. Add and test the bounded graceful path through the sanctioned wrapper, with
   no force escalation and a manifest published only after verified replacement.
4. Run the repaired probe against a disposable, isolated real-service instance;
   validate common origin, interference rejection and exact cleanup without
   using live handles as an inert test fixture.
5. Re-audit foreign activity, capture the exact candidate and configuration,
   perform required signed handoffs, and deploy only the reviewed scope. Verify
   fresh PID/start, executable/helper identities, ports, readiness, continuity,
   logs and advancing finite telemetry. Then observe ordinary activity without
   requesting confirmation of improvement.

## Coordination and Git

Controller maintenance hold 357 was acquired by `codex-astra-interactive`, with
no active steward lease. Read-only bridge preflight passed: no cooperative
session, source-write age 766.2 seconds against the unchanged 180-second window,
and explicit dirty-source acknowledgement. Passing this check does not settle
the lineage or drain blockers above. An initial invocation with an unsupported
`--operation` option exited during argument parsing; the corrected read-only
invocation passed. No wrapper was executed.

Main and remote main initially matched at `f258d38fe99690a34ee53bb88e8fca2d2cbbe722`.
The research iteration was checkpointed locally as
`2675fa212b15934e1758f73c15ad3bda802800d8`, after reviewing the full staged diff,
four staged Markdown documents and 20 local links. The implementation checkpoint
includes this note; its commit identity is recorded by git rather than a
self-referential hash in its own contents. There is no branch merge needed for
these main-branch commits, and no push is authorized or performed.

The separate feature-map document and all Minime dirty files remain outside
these checkpoints. Candidate source, tests, receipt links, CHANGELOG and the
feedback ledger are reviewed together. This is an interactive checkpoint, not
a productive introspection round or a flywheel archival witness commit.
The scheduler remains paused; releasing the temporary maintenance hold must
not resume it. No reservoir regulation, peer mutation or approval scope changes.

## Checkpoint Verification

The implementation checkpoint's staged source matched the tested worktree
bytes. The complete cached diff, including both retained JSON receipts, was
reviewed. Final checks:

| Check | Result |
| --- | --- |
| Complete bridge library suite | 1,715 passed, zero ignored, 269.43 seconds |
| Bridge binaries, `cargo check --bins` | Passed, 2 minutes 48 seconds |
| Bridge library clippy, warnings denied | Passed |
| Protocol-fake probe suite | 8 passed |
| Actual NumPy history suite | 3 passed |
| Evidence-study runtime | 20 passed |
| Evidence Event Store self-tests | 13 passed |
| Steward controller/projector self-tests | 39 passed |
| Experiential epistemics self-tests | 2 passed |
| Existing deployment-wrapper tests | 7 passed |
| Scoped rustfmt, Python syntax, cached whitespace | Passed |
| Local implementation/documentation links | 32 resolved; repository links also present in index |
| Receipt plan, runner and reservoir-source hashes | Matched |

The 92 Python checks and library suite do not replace the pending real-service
integration test or validate the larger proposed Action protocol. Kernel and
Minime source were unchanged; their full suites were not rerun.

The controller's full-chain status verification during hold 357 passed at V2
sequence 995605, head
`bf6d0cd2cf9490ca0f52fb89337ae5fa5e6b1eba27b3c92704518fca1bca63e1`,
across 16 streams. All four V1 sources were immutable; no active lease,
projection, pending event or evidence error was reported. No productive round
or projection generation was requested.

A later bridge sample advanced to arrival time 1788669394.037646 and reported
`connected_with_current_telemetry`; its timing classification remained
`timing_ambiguous`, which is not reinterpreted as precise event timing. A nearby
Minime sample remained finite at fill 70.92829132080078 percent. These are
natural observations, not effects attributed to undeployed code.

All 20 Minime dirty-file hashes and the separate feature-map hash remained
unchanged. The bridge executable and legacy probe hashes also remained unchanged.
Remote main was rechecked and still matched the initial tip. Scheduled automation
configuration was independently read as `PAUSED`. The temporary hold's release
will be recorded by the controller after the local git checkpoint, not inferred
from a successful test or commit.
