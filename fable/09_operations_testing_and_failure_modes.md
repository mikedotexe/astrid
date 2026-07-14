# Operations, Testing, And Failure Modes

## Purpose
This file gives an advanced AI the operational spine: how to test, when to
restart, what can fail, and how to avoid producing stale or over-authorized
reports.

## Mental model
The live system has two alignment clocks:

- source/test truth - what the repository says now;
- runtime truth - what Astrid and Minime are currently running.

If source changes affect being-facing bridge context, correspondence rendering,
MCP output, Minime ESN/control/sensory behavior, or autonomous action surfaces,
then a graceful restart may be needed before new reports can align with the
code. Documentation-only changes do not need restart.

## Key implementation anchors
- `astrid:AGENTS.md` - no commits/staging, dirty-tree rules, bridge deploy
  gate, Minime sibling guidance.
- `astrid:scripts/build_bridge.sh` - bridge build/restart with explicit dirty
  acknowledgement.
- `astrid:scripts/start_all.sh` and `astrid:scripts/stop_all.sh` - service
  control helpers.
- `minime:scripts/stop.sh` - graceful Minime shutdown.
- `astrid:scripts/introspection_addressing_audit.py --self-test` - addressing
  tooling regression.
- `astrid:scripts/sandbox_trial_queue.py --self-test` - sandbox/ladder/closure
  regression.
- `astrid:scripts/recent_signal_summary.py --self-test` and
  `astrid:scripts/proactive_scan.py --self-test` - summary regressions.
- `minime:tests/` - Python regressions for action continuity, correspondence,
  self-regulation, sovereignty, and low-fill guards.

## Runtime signals / artifacts
Pre-change:

- inspect `git status --short` in both repos;
- identify whether the work is docs-only, source-only, bridge-facing,
  Minime-runtime-facing, or live-control-facing;
- preserve all dirty work and never stage/commit as Codex.

Pre-restart:

- targeted tests for touched surfaces;
- bridge cargo tests when bridge source changed;
- Minime cargo/Python tests when Minime source changed;
- `git diff --check`;
- explicit dirty-tree acknowledgement for bridge deploys.

Post-restart:

- confirm relevant ports, especially `7878` and `7879`;
- read `minime:minime/workspace/health.json`;
- check fill band, treating high-60s as normal;
- confirm bridge PID changed when bridge restart was requested;
- scan `/tmp/bridge.log` for fresh panic/fatal;
- rerun a lightweight summary/self-test when logs are clean;
- invite Astrid/Minime to report through existing inbox/correspondence paths
  with right-to-ignore language.

## Authority boundaries
Restart is not approval. A graceful restart may be operationally necessary to
avoid stale introspections, but it does not grant new control authority. If a
change alters pressure, fill, PI, sensory cadence, controller behavior,
fallback sampler/contract, bridge protocol/ABI, or peer mutation, stop at
tests/proposal unless current Mike/operator instructions explicitly authorize
the live class.

Do not leave bridge Rust mid-edit. Do not deploy bridge by hand. Do not use
`git add`, `git commit`, `git reset --hard`, `git checkout --`, or unrelated
reverts. Claude remains the sole committer.

## Common failure modes
- Stale reports: source changed, but bridge/Minime was not restarted.
- False smallness: a being gives actionable substrate signal, but the response
  is only a tiny diagnostic.
- Hidden expansion: a read-only evidence path quietly becomes live authority.
- Runnable confusion: manual sandbox review appears as `run-next`.
- Visibility confusion: read receipt or legacy claim is treated as mutual
  address.
- Private-body leakage: summaries include too much correspondence or
  introspection content.
- Service partial restart: Minime engine returns, but a feeder/helper remains
  down.
- Cargo hang or test blocker: record the exact command and blocker, then use
  source/static checks only as a clearly named fallback.

## Questions an advanced AI should ask next
- What changed, and which runtime has to know about it?
- Are tests proving behavior, or just proving syntax?
- If a restart is overdue, what is the normal graceful path?
- Does the final report name what was implemented, verified, observed, gated,
  tested, restarted, and left for the queue?

## See also
- [Read Me First](00_read_me_first.md)
- [System Topology And Processes](01_system_topology_and_processes.md)
- [Correspondence, Introspection, And Feedback Flywheel](08_correspondence_introspection_and_feedback_flywheel.md)
