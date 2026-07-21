# Full-read summary: introspection_minime_regulator_1784586129

Astrid reads the documented move from the old token-oriented PD path to the
current PI fill/lambda regulation as a shift from gating quantity to shaping
persistent state. Current PI source includes bounded accumulators,
back-calculation, actuator-aware conditional integration, and a hard clamp.
Focused Minime regulator tests passed 86 cases, including deterministic
counterfactual and PI-pressure wiring evidence.

A single current health snapshot showed pressure near 0.22 amid high entropy
and dense viscosity, but cannot establish hunting or identify viscosity as its
cause. A sustained read-only replay is the grounded next step. Kp/Ki, damping,
gate, porosity, or lambda-target changes remain Tier 5.

Evidence: `minime/src/regulator/core/pi.rs`, regulator tests, and
`telemetry_runtime_observation.json`.
