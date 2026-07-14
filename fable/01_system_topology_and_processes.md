# System Topology And Processes

## Purpose
This file explains the running shape of Astrid plus Minime: which processes
exist, which ports matter, how data moves, and where operational authority lives.
It is the first map to consult before deciding whether a change is merely
documentation, source-only, bridge-local, Minime runtime-facing, or a live
restart/deploy event.

## Mental model
Think in four cooperating layers:

- Astrid kernel and capsules hold symbolic authority: files, tools, IPC,
  approvals, audit, capsule lifecycle.
- The spectral bridge translates between Astrid's symbolic world and Minime's
  telemetry/sensory/control surfaces.
- Minime Rust engine is the physiological loop: 66D intake, 128D ESN,
  covariance/eigen telemetry, stable-core PI/homeostasis, regulator state.
- Minime Python layer is the reflective/autonomous layer: dialogue, journals,
  experiments, action continuity, NEXT decisions, and durable records.

The coupling is useful only when each layer remains legible. A bridge message is
not a Minime decision; a Minime report is not automatically a controller change;
an operator restart is not approval for new live authority.

## Key implementation anchors
- `astrid:AGENTS.md` - shared-tree rules, no commits by Codex, bridge deploy
  path, dirty-tree acknowledgement requirements.
- `astrid:scripts/build_bridge.sh` - only approved bridge build/restart path.
- `astrid:scripts/start_all.sh` and `astrid:scripts/stop_all.sh` - service
  startup/shutdown helpers used around the bridge and Minime.
- `astrid:capsules/spectral-bridge/src/mcp.rs` - MCP tool surface, including
  telemetry, semantic sends, bounded control sends, and authority checks.
- `minime:README.md` - four-layer Minime shape and ports.
- `minime:minime/src/main.rs` - runtime args, websocket servers, homeostat,
  stable-core loop, EigenPacket.
- `minime:minime/src/sensory_ws.rs` and `minime:host-sensory/src/app.rs` -
  sensory input path and host-generated fallback sources.
- `minime:scripts/stop.sh` - graceful Minime shutdown path.

## Runtime signals / artifacts
Minime ports:

- `7878` - telemetry stream, JSON EigenPacket/spectral state.
- `7879` - sensory/control input, JSON SensoryMsg.
- `7880` - optional GPU video input, binary frames.
- `7881` - optional reservoir/holographic telemetry.
- `8080` - optional holographic HTTP API.

Important process facts:

- Astrid bridge source changes do not affect the live bridge until built through
  `astrid:scripts/build_bridge.sh`.
- Minime Rust source changes do not affect the live Minime engine until release
  binaries are built and the launchd-managed slice is restarted.
- `bash astrid:scripts/start_all.sh --minime-only` may not restore every
  launchd helper in every state; recent operations needed an explicit check for
  the Minime feeder after restart.
- High-60s Minime fill is normal stable-core hold, not distress.

## Authority boundaries
Normal source edits are not live authority. A graceful restart is operational
alignment so the beings can report on the current code; it is not approval for
new pressure/fill/PI/controller/sensory-cadence/fallback/protocol behavior.

Bridge deploys must use the bridge gate:

```bash
scripts/build_bridge.sh --ack "Mike approved deploying local dirty Astrid bridge source for <reason>" --restart
```

Minime runtime refresh must use the service-specific build and graceful restart
path, followed by health checks. Avoid ad hoc `cargo build` plus manual
launchctl sequences for the bridge, and avoid `kill -9` for Minime except after
graceful shutdown has already failed.

## Questions an advanced AI should ask next
- Am I changing documentation, source-only diagnostics, bridge rendering, or
  live control-facing behavior?
- Does this change need a bridge build or Minime restart before being reports
  can align with it?
- Which health checks prove the restart was clean?
- Which process is allowed to carry this authority: Codex, Claude, Mike, the
  bridge gate, the Minime service path, or no one yet?

## See also
- [Stable Core, PI Controller, And Homeostasis](04_stable_core_pi_controller_and_homeostasis.md)
- [Astrid Bridge, Capsule, And Tooling](07_astrid_bridge_capsule_and_tooling.md)
- [Operations, Testing, And Failure Modes](09_operations_testing_and_failure_modes.md)
