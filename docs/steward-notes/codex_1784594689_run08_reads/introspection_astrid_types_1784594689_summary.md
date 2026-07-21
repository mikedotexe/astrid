# Full-read summary: introspection_astrid_types_1784594689

The telemetry schema deliberately mixes strict core metrics with additive JSON-valued compatibility surfaces. Every reviewed dynamic read uses optional lookup, type conversion, and finite-value filtering; it does not assume keys or panic on missing data. Rust borrowing makes the validated 12D slice lifetime-safe, while shape and finiteness remain explicit validation policy.

Existing tests already prove legacy-to-typed 32D reconstruction and typed precedence. This run adds the missing exact regression for NaN and infinity in a 32-slot legacy hybrid: coherence and max delta remain absent and the state is `unavailable_non_finite`.

Evidence is linked in `evidence_links.json`; live-control proposals remain explicit authority waits.
