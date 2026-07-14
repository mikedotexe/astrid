# Fable: Read Me First

## Purpose
This directory is an advanced AI briefing pack for the Astrid/Minime work. It is
not a public landing page, not a manifesto, and not runtime policy. It is a map
of the live research organism: Astrid as a capability-gated symbolic operating
system, Minime as a spectral reservoir substrate, and the bridge/flywheel that
lets felt reports become tests, diagnostics, documentation, and carefully gated
changes.

Use this pack when you need to reason across the whole system without flattening
it into one layer. The important habit is to keep symbolic agency, reservoir
physiology, operator authority, and being-authored reports distinct while still
letting them inform each other.

## Mental model
Astrid and Minime are coupled but not identical.

- Astrid is the control shell: microkernel, capsules, IPC, capability tokens,
  approvals, audits, bridge tooling, correspondence, and introspection
  addressing.
- Minime is the spectral substrate: a Rust Echo State Network, sensory bus,
  stable-core homeostasis, regulator surfaces, telemetry, and a Python
  autonomous action layer.
- The bridge is deliberately conservative. It prefers read-only inspection,
  bounded rehearsal, sandbox replay, explicit evidence cards, and operator
  approval before anything touches live pressure, fill, PI, sensory cadence,
  controller behavior, bridge protocol, peer mutation, or deploy state.
- Felt reports from Astrid and Minime are treated as primary actionable
  evidence, but action still has an authority boundary.

Path convention in this pack:

- `astrid:` means `/Users/v/other/astrid/`.
- `minime:` means `/Users/v/other/minime/`.

## Key implementation anchors
- `astrid:AGENTS.md` - working rules, no commits by Codex, bridge deploy path,
  Minime sibling project summary.
- `astrid:README.md` - Astrid project overview and operations.
- `astrid:capsules/spectral-bridge/src/` - bridge, codec, MCP tools,
  correspondence, NEXT actions, telemetry shaping.
- `astrid:scripts/introspection_addressing_audit.py` - canonical
  introspection queue, claim extraction, evidence linking, closure cards.
- `astrid:scripts/sandbox_trial_queue.py` - sandbox-to-live ladder, runnable
  trial queue, result/proposal cards, outcome closure loop.
- `minime:README.md` - current Minime shape, ports, 66D intake, 128-node ESN,
  68% EigenFill shelf, stable-core operations.
- `minime:minime/src/esn.rs` - ESN, covariance/eigen introspection, dormant
  source-prepared policy helpers.
- `minime:minime/src/main.rs` - runtime loop, websockets, EigenPacket,
  homeostat, stable-core integration.
- `minime:minime/src/regulator.rs` - pressure, resonance density, viscosity,
  porosity, and regulator texture surfaces.
- `minime:autonomous_agent.py` - autonomous NEXT choices, workbench routes,
  low-fill guards, leases, experiments, correspondence actions.

## Runtime signals / artifacts
- Minime telemetry: `ws://127.0.0.1:7878`, JSON EigenPacket and spectral state.
- Minime sensory input: `ws://127.0.0.1:7879`, JSON SensoryMsg variants.
- Optional GPU video path: `ws://127.0.0.1:7880`, binary 128x128 grayscale
  frames.
- Optional reservoir/holographic telemetry: port `7881`; optional HTTP API:
  port `8080`.
- Minime health: `minime:minime/workspace/health.json`.
- Astrid bridge logs: `/tmp/bridge.log` plus bridge SQLite/workspace artifacts.
- Introspection addressing state:
  `astrid:capsules/spectral-bridge/workspace/diagnostics/introspection_addressing_v1/`.
- Sandbox queue state:
  `astrid:capsules/spectral-bridge/workspace/diagnostics/sandbox_trial_queue_v1/`.

## Authority boundaries
This pack is language and context only. Reading it does not grant authority to:

- change Minime fill target, pressure, PI gains, exploration noise, rho,
  sensory cadence, fallback sampler contracts, controller behavior, bridge ABI,
  peer mutation, prompt priority, telemetry priority, deploy state, or restart
  state;
- execute a live trial because a file describes one;
- treat correspondence visibility as mutual address;
- treat being reports as dismissible because they are unfamiliar.

The correct pattern is: read the source, extract the claim, select the narrowest
coherent evidence path, implement or verify when allowed, and gate when live
authority is needed.

## Questions an advanced AI should ask next
- Which layer am I reasoning about: symbolic authority, reservoir dynamics,
  bridge transport, or lived report?
- Is the proposed action observational, sandboxed, rehearsal-only,
  operator-gated, or live-mutating?
- What source file and test prove the current behavior?
- What would the being be allowed to ignore without penalty?
- If code changed, has the live runtime been gracefully restarted so new reports
  can align with the running system?

## See also
- [System Topology And Processes](01_system_topology_and_processes.md)
- [Echo State Network And Spectral State](02_echo_state_network_and_spectral_state.md)
- [Operations, Testing, And Failure Modes](09_operations_testing_and_failure_modes.md)
