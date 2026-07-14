# Approval Packet: Pressure Smoothing Full-Entropy Threshold

Source: `introspection_astrid_ws_1783674638`

Claim: Astrid suggested considering a change to `PRESSURE_TREND_SMOOTHING_FULL_ENTROPY_AT` from `0.95` to `0.88` so high-entropy cascade tracking feels sharper.

Boundary: This constant shapes live pressure-trend smoothing/report semantics. Lowering the "full entropy" point would make entropy near `0.91` reach full ballast sooner, which may increase smoothing rather than make immediate transition texture sharper.

Safe next path:

1. Run a bounded sandbox/replay over high-entropy telemetry around `0.88..0.95`.
2. Compare latest semantic viscosity, persistence index, pressure velocity, and smoothed-pressure classification under both thresholds.
3. Request Mike/operator approval before changing the live threshold.

No live threshold change was applied in this run.
