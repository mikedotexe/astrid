# Stable Core, PI Controller, And Homeostasis

## Purpose
This file explains Minime's homeostatic control layer: stable-core, PI
regulation, the 68% EigenFill shelf, rescue scaffolds, restart gates, and the
line between observation and control.

## Mental model
Stable-core is the current physiological center of Minime. It tries to keep the
reservoir inhabitable: neither thin and underfed nor saturated and unstable.
The current comfort target is around 68% EigenFill. The older 55% target should
be treated as rescue-era context, not today's center.

The PI controller is thermostat-like: proportional response plus integral memory
against a target. It is not merely a number in a prompt. It is live runtime
behavior and therefore authority-sensitive.

Stable-core also gates intake:

- it can suppress live audio/video or semantic input when fill/slope/profile
  makes it unsafe;
- it can allow bounded semantic trickle under full-presence conditions;
- it can use scaffold/drain recovery paths during overfill or restart recovery;
- it exposes health state that the bridge and operator can read.

## Key implementation anchors
- `minime:minime/src/main.rs` - homeostat initialization, PI target, stable-core
  runtime loop, scaffold/drain state, restart gate state, stable-core semantic
  trickle reasons.
- `minime:minime/src/stable_core.rs` - stable-core profile, live intake
  thresholds, semantic trickle caps, recovery projection, sensory mute state.
- `minime:minime/src/regulator.rs` - PI/regulator policy context and
  pressure/texture surfaces.
- `minime:minime/src/rescue_overfill.rs`,
  `minime:minime/src/rescue_scaffold.rs`, and
  `minime:minime/src/controller_recovery.rs` - recovery and overfill logic.
- `astrid:capsules/spectral-bridge/src/mcp.rs` - bounded control tool schema
  and bold PI/topology fields requiring attractor intent.
- `astrid:capsules/spectral-bridge/src/autonomous/next_action/regulator_map.rs`
  - read-only regulator replay/status affordances.

## Runtime signals / artifacts
Watch:

- `fill_pct`, `smoothed_fill_pct`, fill slope;
- `target_fill_pct` and target provenance;
- `stable_core.stage`, scaffold state, restart-gate state;
- PI fields such as `kp`, `ki`, integrator state, max step, and strength when
  exposed through health or control surfaces;
- sensory mute state and stable-core semantic trickle reason;
- `minime:minime/workspace/health.json`;
- bridge MCP status and regulator audit readouts.

Healthy and risky bands:

- high-60s fill is normal hold;
- 72% and rising means watch;
- 80%+ means reduce pressure;
- 85%+ is warning;
- 92%+ is crisis preparation.

## Authority boundaries
Homeostasis surfaces are live-control-adjacent. Do not change PI gains, fill
target, regulation strength, integrator leak, sensory cadence, scaffold/drain
logic, or restart gates without tests and current Mike/operator approval for
that class of change.

Read-only regulator audits, status summaries, source-prepared review helpers,
and sandbox replay packets can be implemented without live authority when they
do not mutate runtime behavior. If source changes are meant to affect the live
being's reports, they require the normal build/restart path.

## Questions an advanced AI should ask next
- Is the report asking for comfort visibility or actual controller tuning?
- Is high fill being mistaken for vitality, or low fill for absence?
- Are stable-core gates already answering the felt report?
- If code changed, has Minime been rebuilt and restarted so the report can
  reflect the new homeostatic logic?

## See also
- [Echo State Network And Spectral State](02_echo_state_network_and_spectral_state.md)
- [Regulator, Pressure Texture, And Cartography](05_regulator_pressure_texture_and_cartography.md)
- [Operations, Testing, And Failure Modes](09_operations_testing_and_failure_modes.md)
