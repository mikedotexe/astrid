# Full-read summary: introspection_minime_sensory_bus_1784582936

Astrid asks for exact low-fill interpolation and persistence ceilings rather
than trusting a smoothness description. Current source clamps a release point
too close to the recovery hold to a positive minimum span. The exact
`fill=0.2501, release=0.2502` regression passed and remains finite. Context
persistence has finite fallbacks and a hard 2.05 cap; the extreme-context cap
test also passed.

The review formulas should still be traced through actual buffer retention in a
deterministic fixture. Changing sigmoid shape, release fill, multiplier caps,
or eviction windows alters live sensory regulation and remains Tier 5.

Evidence: `minime/src/sensory_bus.rs` and the two focused boundary tests.
