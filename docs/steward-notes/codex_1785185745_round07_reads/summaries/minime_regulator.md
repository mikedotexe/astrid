Astrid correctly identifies `regulator/core.rs` as an include manifest and
correctly distinguishes retained PD types from the exclusively active PI
mode. The report's open-gate, pressure, packing, and porosity values are
telemetry context, not evidence that the regulator is actively preventing
collapse or that a band-stop filter causes felt viscosity. Current viscosity
and pressure-source surfaces are explicitly read-only reviews. A bounded
gate/pressure/viscosity alignment review is available without manipulating
porosity; filter, gate, PI, viscosity, density-gradient, and regulator changes
remain Tier 5 waits.
