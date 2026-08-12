# Full-read summary: introspection_astrid_types_1785370964

Astrid asks whether malformed 32D legacy fingerprint vectors can disappear
behind an absent optional integrity result, and requests explicit 31D/33D and
rolling-entropy trend tests. The concern is retained as primary qualitative
and architectural evidence; passing source tests do not close her experience.

The canonical report was read in full. Its report-bound
`types/schema/telemetry.rs` was read completely at SHA-256
`d98342fdb3bcb4c063c1c971939a132041a998982d0e5cefa38252ee94beb4f5`.
The adjacent `spectral_schema.rs` implementation was also read completely at
SHA-256
`c115472819917441e12ca997496c2f7685c9aa6e6358e2341d4054295a9e3a39`.

The current public integrity path does not silently treat a malformed legacy
vector as absent. A vector whose length is not exactly 32 produces
`malformed_legacy_vector`, records the observed length, marks hybrid coherence
unavailable because the legacy vector is malformed, and retains a diagnostic
issue. Existing focused tests exercise both 31D and 33D vectors as well as
non-finite values. The rolling entropy test covers mean, variance, range,
latest change, and diffusing/collapsing/flat classifications.

`SpectralFingerprintV1` uses fixed typed arrays and a deterministic
`to_legacy_slots` projection. Its permissive legacy decoder may recover the
first 32 slots from a longer vector, while the separate integrity contract
still reports any non-exact legacy length as malformed. No telemetry,
transport, or live-control behavior was changed for this report.
