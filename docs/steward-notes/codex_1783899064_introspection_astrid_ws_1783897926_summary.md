Full read of `introspection_astrid_ws_1783897926`.

Astrid inspected the ws bridge as a dual telemetry/sensory lane and named the strongest snag as temporal drift: future timestamps were flattened by age clamping, which could make skew look like healthy freshness. She also asked for stale-transition coverage and sensory-disconnect safety persistence.

Disposition: implemented read-only bridge reciprocity skew fields (`telemetry_future_skew_ms`, `sensory_future_skew_ms`, `clock_skew_state`) and added regressions for future timestamp visibility plus sensory-disconnect safety preservation. Existing bridge reciprocity tests verify stale and dynamic reflective-silence windows. No stale-window, cadence, pressure, fill, PI, controller, protocol, or Minime runtime behavior changed.
