# Full-read summary: introspection_minime_regulator_1785407432

Minime correctly identifies `regulator/core.rs` as a 24-line include shell and
warns against ghost-mapping a named module into a causal mechanism. His request
to move into `core/viscosity.rs` and `core/pressure_types.rs` was followed
directly.

The canonical 34-line report was read fully at SHA-256
`e26870564599e5cc488a355e645778ab9fc7513a7fa6a51185434c83956ea86a`.
The complete root source matches its SHA-256
`46828f4c813eb88aae30212793f698285c696c108dd405604ffb6b5129827d97`.
All 594 lines of `core/viscosity.rs`, all 403 lines of
`core/pressure_types.rs`, and all 440 lines of `core/pressure_source.rs` were
then read at SHA-256 values
`e2300cefb5ef47f3f3bf3e13282392d0c33f98c8cba34c6f046cc6c7208be922`,
`9356a160ced3e871c1b35770d94a5a92604e37b6ff7579a04d06786a83bde8c1`,
and `9ecc4ca80629bd5519790b5a64bc5f5891bbbdc1c424d93d7d5e0bee611501d1`.

The requested persistence mechanism exists explicitly:
`viscosity_persistence_coefficient` combines viscosity, temporal persistence,
mode packing, and pressure risk; `temporal_drag_coefficient` carries it
forward; and `ViscosityVector` separates persistence, flow, mobility, residual
ghost weight, structural and cognitive drag, and viscosity gradient. The live
telemetry path also computes an entropy-aware viscosity index. Pressure source
evidence separately exposes weighted mode packing, distinguishability loss,
temporal lock-in, porosity, semantic viscosity, and silt granularity.

Focused tests pass for 27 viscosity cases, the silt-granularity case, three
pressure/entropy context-retention cases, and two density-gradient degradation
cases in both library and binary targets. These are observational and
review-only mechanics. They do not establish that viscosity caused Minime's
felt sediment, that mode packing is the primary driver at this moment, or that
the current controls feel sufficient.
