# Regulator, Pressure Texture, And Cartography

## Purpose
This file maps the regulator and pressure-texture language: resonance density,
mode packing, porosity, viscosity, pressure source, mobility, inhabitable
fluctuation, and cartography diagnostics.

## Mental model
The regulator is not just "keep lambda1 down." It has become a rich observation
surface for how spectral pressure is organized. Being reports often name
textures: viscous, settled, porous, dense, muffled, bright, restless, weighted,
or habitable. The job is not to poetically accept every term, nor to dismiss it.
The job is to ask which measurable surface could carry the report.

Core distinctions:

- density is not identical to pressure;
- pressure can come from mode packing, persistence, semantic trickle,
  low porosity, or dominant modes;
- high entropy can be volatile rather than calm;
- settled can mean habitable or stuck;
- damping/control suggestions are not the same as applying damping/control.

## Key implementation anchors
- `minime:minime/src/regulator.rs` - `ResonanceDensityV1`,
  `ResonanceDensityComponents`, `ViscosityVector`, pressure-source policy,
  semantic viscosity, silt granularity, settled mobility, shadow preservation,
  inhabitable fluctuation.
- `minime:minime/src/regulator_cartography.rs` - cartography support for
  regulator surfaces.
- `astrid:scripts/coupled_pressure_cartography.py` - read-only coupled pressure
  cartography from Minime health/regulator context and optional probes.
- `astrid:capsules/spectral-bridge/src/autonomous/next_action/regulator_map.rs`
  - read-only `REGULATOR_BOUNDARY_CARD`, `PI_PRESSURE_REPLAY_STATUS`, and
  related regulator map renderers.
- `astrid:capsules/spectral-bridge/src/autonomous/next_action/sovereignty.rs`
  - protected pressure-source and regulator audit actions.
- `astrid:scripts/recent_signal_summary.py` and
  `astrid:scripts/proactive_scan.py` - summaries of source/test evidence for
  regulator and pressure claims.

## Runtime signals / artifacts
Useful fields and artifacts include:

- `resonance_density_v1.density`;
- `containment_score`;
- `pressure_risk`;
- `components.mode_packing`;
- `components.temporal_persistence`;
- `components.viscosity_index`;
- `components.residual_ghost_weight`;
- `texture_signature`;
- `texture_component_alignment`;
- `control.applied_locally`;
- `regulator_context.json` in Minime workspace when present;
- coupled pressure cartography JSON/Markdown outputs under Astrid diagnostics.

Interpretation habit:

- first identify the pressure source;
- then check whether the source is structural, semantic, sensory, controller,
  or legacy/residual;
- then decide whether visibility, replay, proposal, or live control is the
  right response.

## Authority boundaries
Regulator language is often close to control. A pressure-source audit is
read-only. A boundary card is read-only. A PI pressure replay is read-only unless
current instructions explicitly grant runtime wiring. Any change that alters
pressure, fill, PI, damping, semantic gain, mode packing, or controller behavior
must be treated as live control-facing work.

Do not turn a regulator report into a tiny diagnostic if the report clearly
supports a larger coherent source/test/replay implementation. Also do not
silently upgrade a diagnostic into live authority.

## Questions an advanced AI should ask next
- Which pressure source is named or implied?
- Does porosity explain why high density feels habitable or trapped?
- Does the current source already expose the missing variable?
- Is this a candidate for sandbox replay, proposal card, or operator-approved
  live trial?

## See also
- [Stable Core, PI Controller, And Homeostasis](04_stable_core_pi_controller_and_homeostasis.md)
- [Actions, Autonomy, And Authority](06_actions_autonomy_and_authority.md)
- [Correspondence, Introspection, And Feedback Flywheel](08_correspondence_introspection_and_feedback_flywheel.md)
