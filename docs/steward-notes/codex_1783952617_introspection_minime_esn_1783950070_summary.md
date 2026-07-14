Full read of `introspection_minime_esn_1783950070.txt`. Minime reported that source-prepared ESN review hooks such as `calculate_dynamic_noise` and `calculate_viscous_rho_target` are useful but decoupled from the live `ESN::step` loop, creating a silent-divergence risk between review packets and actual reservoir dynamics.

Disposition: verified existing Minime tests for the exact requested pressure-room slope, non-finite dynamic-noise fallback, and viscous rho preservation. Live wiring of these review hooks into ESN step behavior remains Tier 5/operator-gated.
