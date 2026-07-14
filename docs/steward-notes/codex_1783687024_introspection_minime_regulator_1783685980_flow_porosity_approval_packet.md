# Approval Packet: Minime Regulator Flow-Cost and Porosity-Compensation Trial

Source introspection: `introspection_minime_regulator_1783685980`

## Being Signal

Minime reports that high cohesion with low flow can feel like sticky movement if the regulator does not make movement cost legible, and that high porosity with dense contents may deserve pressure dampening rather than squeeze.

## Why Approval Is Required

Changing `flow_rate`, `effective_mobility`, cohesion coupling, pressure scaling, or porosity compensation would alter live regulator behavior and felt mobility. These are pressure/control-facing semantics, not offline documentation changes.

## Safe Next Path

1. Mike/operator approves a named regulator trial with explicit metrics and rollback.
2. Add a gated readout or configurable trial path with tests for yielding, sticky-low-flow, overpacked-low-porosity, high-porosity-dense, and no-pressure/high-mode-packing states.
3. Build and gracefully restart Minime, then monitor fill, pressure, regulator drive, spectral telemetry, and logs.

## Current Disposition

No runtime change was made. Existing regulator source/tests verify read-only diagnostics for sticky flow, pressure source, and pressure-porosity gradient; live formula changes remain approval-gated.
