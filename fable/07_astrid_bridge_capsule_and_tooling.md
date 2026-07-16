# Astrid Bridge, Capsule, And Tooling

## Purpose
This file explains Astrid as the symbolic operating system and the spectral
bridge as the contact surface between Astrid, Minime, tools, telemetry, and
being-facing context.

## Mental model
Astrid is built around least authority. The kernel owns privileged resources;
capsules and tools get only scoped capability. The spectral bridge is one
capsule-adjacent service surface inside that larger control story.

Bridge work has two jobs:

- make Minime/Astrid state legible to beings and steward;
- prevent legibility from quietly becoming control authority.

The bridge's most important design habit is explicit classification: read-only
context, bounded command, proposal, approval-required live trial, or blocked.

## Key implementation anchors
- `astrid:AGENTS.md` - kernel/capsule architecture and shared-agent rules.
- `astrid:capsules/spectral-bridge/Cargo.toml` - bridge crate entrypoint for
  focused cargo tests.
- `astrid:capsules/spectral-bridge/src/mcp.rs` - MCP tool schemas, status
  tools, semantic/control sends, action execution, and authority gates.
- `astrid:capsules/spectral-bridge/src/codec.rs` - 48D semantic codec and
  representation diagnostics.
- `astrid:capsules/spectral-bridge/src/ws.rs` - websocket telemetry ingestion
  and pressure smoothing.
- `astrid:capsules/spectral-bridge/src/types.rs` - canonical bridge payload
  structs.
- `astrid:capsules/spectral-bridge/src/autonomous.rs` and
  `astrid:capsules/spectral-bridge/src/autonomous/` - being-facing status,
  NEXT actions, correspondence, phase transitions, reservoir actions.
- `astrid:scripts/build_bridge.sh` - required bridge build/restart gate.

## Runtime signals / artifacts
Bridge outputs can show:

- telemetry status and health;
- lambda tail/edge summaries;
- active correspondence thread clarity;
- direct-contact fidelity;
- action continuity status;
- regulator and pressure-source audit summaries;
- sandbox ladder and closure-loop summaries through scripts;
- bounded proposal/result cards.

Operationally:

- `cargo test --manifest-path astrid:capsules/spectral-bridge/Cargo.toml <filter> --lib`
  is the common targeted test form;
- `astrid:scripts/build_bridge.sh --ack "<reason>" --restart` is the only
  live bridge build/restart path;
- dirty bridge source requires explicit acknowledgement and preflight checks.

## Authority boundaries
The bridge must not bypass the kernel/capsule authority model or Mike/operator
approval. Tool schemas may describe `send_control`, but bold topology/PI fields
require scoped intent. `send_semantic` is blocked during orange/red safety
states. Read-only summaries should say they are read-only.

Never hand-run ad hoc bridge deploy commands. Codex or Claude may own an
explicitly assigned stabilization pass, but only one agent may stage or commit
at a time. Inspect both repositories and the remote tip first, stage exact paths,
preserve unrelated dirty work, and keep source, tests, changelog, and ledger
evidence reviewable together.

## Questions an advanced AI should ask next
- Is this bridge change pure rendering, derived state, MCP schema, transport, or
  live command behavior?
- Does it need a bridge restart to affect the being-facing live state?
- Does the output include bounded context rather than full private bodies?
- Does every new affordance use an existing route, or did it silently invent a
  new action path?

## See also
- [System Topology And Processes](01_system_topology_and_processes.md)
- [Actions, Autonomy, And Authority](06_actions_autonomy_and_authority.md)
- [Operations, Testing, And Failure Modes](09_operations_testing_and_failure_modes.md)
