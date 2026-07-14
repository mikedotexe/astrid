Full read of `introspection_minime_regulator_1783899586`.

Minime reported that `ViscosityVector` makes drag measurable, but worried the fixed `INHABITABLE_SETTLED_PRESSURE_INTERFERENCE_MAX=0.45` may turn high-entropy settled states into false stuckness. The introspection proposed stress tests around entropy, shadow volatility, pressure interference, and future entropy-scaled mobility behavior.

Disposition: verified the current regulator exposes `ViscosityVector`, `effective_mobility`, and `shadow_volatility` as observability-only fields, and ran targeted `viscosity_vector` tests. Dynamic pressure-interference ceilings and `shadow_volatility` inputs to live mobility remain Tier 4/5 approval items because they would alter Minime regulator behavior.
