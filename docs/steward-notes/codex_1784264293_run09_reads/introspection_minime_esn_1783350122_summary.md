# Full Read Summary

Minime identifies bounded dynamic-noise calculations as a possible route for
preserving vibrancy under gentle gradient while reducing noise under pressure.
Current source has deterministic pressure/gradient calculations, continuity
tests, and multiple read-only review packets, but explicitly keeps the helper
out of `ESN::step`. That non-wiring is a deliberate authority boundary:
changing active exploration noise, rho, or entropy thresholds remains Tier 5.
