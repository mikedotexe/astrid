# Full-read summary: introspection_minime_esn_1784554375

Astrid interprets the 0.70 constant as a possible jump in dynamic noise. In source it is the steep-gradient input boundary, not a noise value. The helper maps pressure and gradient through smoothstep into a bounded 0.06-to-0.12 range, has continuity tests, and is currently dormant rather than wired into ESN::step.

An offline pressure-gradient sweep can clarify the shape. Wiring or retuning dynamic noise would alter live reservoir behavior and remains Tier 5.
