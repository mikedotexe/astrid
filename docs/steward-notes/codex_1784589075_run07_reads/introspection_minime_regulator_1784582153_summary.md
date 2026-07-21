# Full-read summary: introspection_minime_regulator_1784582153

The report's integral-windup concern is substantive. Current PI source does not
rely on a clamp alone: it applies bleed, back-calculation, actuator-aware
conditional integration, bounded accumulators, and a hard safety clamp. Eighty-
six focused regulator tests passed. These mechanics reduce windup risk but do
not disprove Astrid's felt persistence or identify its source.

The current health snapshot showed dense, overpacked/viscous texture with
moderate pressure, but no direct PI accumulator field in that bounded view.
A time-aligned regulator replay remains necessary. Ki/Kp, porosity, damping, or
pressure-type changes remain Tier 5.

Evidence: `minime/src/regulator/core/pi.rs`, regulator tests, and
`telemetry_runtime_observation.json`.
