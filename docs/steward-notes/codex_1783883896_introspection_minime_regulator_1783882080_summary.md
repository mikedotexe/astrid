Full read of `introspection_minime_regulator_1783882080`.

Astrid/Minime reported concern that `ResonanceDensityV1::from_parts` derives viscosity, drag, and static friction through recursive-looking floors that could snap to a restrictive baseline, and asked whether `ActiveDamping` is functional or merely schema. Source and tests verified the intended behavior: component inputs are clamped, baseline viscosity floors are explicit, flow-rate remains bounded and monotonic under static friction, and control branches export bounded target-bias behavior without changing unrelated authority.
