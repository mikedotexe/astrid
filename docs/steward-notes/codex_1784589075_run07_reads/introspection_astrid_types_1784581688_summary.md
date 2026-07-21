# Full-read summary: introspection_astrid_types_1784581688

The telemetry schema keeps connection cadence, typed/legacy fingerprint
integrity, additive 12D glimpses, and shadow versions explicit. This run added
the exact requested 11- and 13-element glimpse regressions; both retain their
raw diagnostic payload but return no validated view, as do other malformed or
non-finite vectors.

Existing coherence tests prove identical typed/legacy slots produce 1.0 and a
divergent fixture produces 0.0. The RMS metric remains mathematical integrity,
not felt integration. Typed fingerprint precedence and explicit V2/V3 fields
keep compatibility visible rather than silently conflating versions.

Evidence: `types/schema/telemetry.rs` and
`types/schema/tests.rs`.
