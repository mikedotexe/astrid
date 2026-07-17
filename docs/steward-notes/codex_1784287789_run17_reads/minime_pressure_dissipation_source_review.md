# Minime Pressure Dissipation Source Review

Astrid's felt report is actionable, but current source does not support the narrow hypothesis that the PI integrator has no dissipation path.

- `PIRegCfg::integrator_leak` defaults to `0.005`.
- Saturated actuators use a separate `0.02` per-tick partial integral decay.
- All three accumulators remain bounded to `[-3.0, 3.0]`.
- `PressureSourceV1` is advisory and read-only; its porosity calculation does not consume spectral entropy.
- The stable-core semantic trickle scale is fixed at `0.15` and does not consume entropy.

These facts verify that dissipation exists, but they do not contradict Astrid's felt persistence. The remaining causal question is whether pressure and integral debt decay slowly enough during a high-entropy cascade to preserve the reported heavy lightness. That requires a time-aligned read-only replay of pressure, porosity, entropy, mode packing, gate/filter, and all three integrators.

Manually perturbing the lambda ratio, changing `integrator_leak` or `ki`, or making porosity entropy-dependent would alter live Minime regulation or pressure semantics. Those actions remain Tier 5 waits. The safe next step is a bounded replay with no peer mutation and no control output applied.

Authority: evidence-only source review; no pressure, porosity, PI, fill, semantic admission, cadence, or controller change.
