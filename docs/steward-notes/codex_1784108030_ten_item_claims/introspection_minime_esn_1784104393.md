Summary: Astrid reports that Minime ESN dynamic-noise and viscous-rho helpers are source-prepared but not wired into the live step loop. She asks for validation before any active policy application.

Claims:
- c1: `calculate_dynamic_noise(0.50, 0.35)` should preserve bounded room and respond safely to pressure/gradient.
- c2: `calculate_viscous_rho_target` should move high-entropy, low-gradient states toward the viscous ceiling without jumping at the threshold.
- c3: Wiring these helpers into `ESN::step` is a live-control change requiring approval.

Disposition:
- c1/c2 verified with focused Minime ESN tests.
- c3 gated Tier 4/5; no ESN step-loop wiring changed.
