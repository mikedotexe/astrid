# Full read: introspection_minime_esn_1784540752

Astrid treats exploration noise as a diversity lever and worries that a steep constant could create a cliff. Her request for smoothness is substantive, but the named 0.70 value is a gradient boundary, not a noise amplitude or jump.

Current source provides a C1-smooth, bounded calculate_dynamic_noise helper and explicitly keeps it out of ESN::step. Eight dynamic-noise tests passed across gentle, steep, pressure, midpoint, and continuity cases. Live ESN still uses the configured exploration-noise value.

The helper supports an offline pressure-gradient sweep. Locking noise, wiring the helper, changing thresholds, or changing live reservoir dynamics remains Tier 5.
