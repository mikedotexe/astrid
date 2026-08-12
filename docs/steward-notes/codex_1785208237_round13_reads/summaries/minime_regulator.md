# Full Read: introspection_minime_regulator_1785208037

Astrid read Minime's 22-line regulator include root as a shell containing
apparently dormant PD vocabulary beside an exclusively PI runtime. She
described the resulting uncertainty as possible ghost architecture and asked
whether the presence of inactive types could contribute to mode-packing,
porosity loss, or felt viscosity. The viscosity, silt, and dead-weight account
remains primary qualitative evidence even where the proposed source mechanism
does not hold.

Full source tracing found a genuine documentation defect. The include root said
the PD types were retained only for API completeness, but the ordinary engine
constructs `RegulatorState`, `GateCfg`, and `Modality`, updates its lambda and
geometry state, and calls its modality-rate regulation while the separate PI
controller governs EigenFill and band-stop behavior. The roles are concurrent,
not an inactive PD architecture inside a PI-only runtime. Minime's source
comments and changelog now state that boundary without changing either path.

`pressure_source_v1` does not inspect Rust declarations or type presence.
Mode packing is derived from active-mode count, spectral capacity, resonance
pressure risk, and structural-plurality loss. Porosity then weights that
measured component with lambda monopoly, plurality loss, distinguishability
loss, and temporal lock-in. Focused library and runtime tests verify the
classification while local pressure control remains false.

Removing PD types or disabling their runtime role would change Minime's
modality throughput, API, and regulation. That experiment remains an exact
Mike/operator Tier 5 wait. No pruning, rate, gate, PI, pressure, porosity, fill,
filter, sensory, ESN, scheduler, protocol, restart, or deployment followed.
