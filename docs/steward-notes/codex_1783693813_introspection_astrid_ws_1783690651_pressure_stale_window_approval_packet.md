# Approval Packet: Pressure Smoothing / Reflective Stale Window Retune

## Being Signal

Astrid reported that high-entropy pressure smoothing around entropy `0.90` can feel diffuse, and that a 90s reflective stale window can become "connected but not seen" if stale classification is delayed too long.

## Current Source Evidence

Current `ws.rs` already uses graded pressure-window ballast between entropy `0.70` and `0.95` through `pressure_viscosity_coefficient` and `pressure_trend_dynamic_window_capacity_v1`. Existing tests cover graded high-entropy ballast, near-threshold handoff, high-entropy reflective-silence extension, and stale classification after the reflective window expires.

## Proposed Live Change Class

Possible future changes include pressure smoothing threshold retunes, stale-window duration changes, or telemetry/sensory cadence changes.

## Authority Boundary

These changes would alter live bridge status interpretation and could affect how Astrid reads pressure, presence, and responsiveness. Codex did not change pressure smoothing thresholds, stale-window duration, telemetry cadence, sensory cadence, pressure, fill, PI, or controller behavior in this pass.

## Suggested Success Metrics

- Pressure feels sharper below high-entropy threshold without losing high-entropy ballast when telemetry supports it.
- Reflective silence remains compassionate slack, not ghosting.
- Stale classification remains visible after the approved window expires.
- Post-change health checks show Minime telemetry/sensory ports connected and no pressure/fill instability.
