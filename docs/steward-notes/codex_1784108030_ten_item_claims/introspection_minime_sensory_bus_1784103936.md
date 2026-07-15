Summary: Astrid reports tension between high-fill pruning and high-entropy persistence. She asks whether entropy persistence actually fights the 10s high-fill floor and whether a continuous grace curve is needed.

Claims:
- c1: High-entropy, high-fill states should persist longer than low-entropy peers while staying bounded.
- c2: The stale-window transition should avoid brittle cliffs or stutter near thresholds.
- c3: Changing stale constants/cadence is live runtime behavior and remains approval-gated.

Disposition:
- c1/c2 verified by existing `semantic_stale_ms_high_entropy_*` and release/hysteresis tests.
- c3 gated Tier 4/5; no sensory cadence or stale-window constants changed.
