# Full-read summary: introspection_astrid_codec_1784580201

Astrid asks for deterministic projection health rather than trusting a fixed
seed. Current evidence records finite column norms, near-zero and dead-column
status, checksum, and a deterministic precision comparison between production
f32 accumulation and an f64 reference. The focused projection-basis test
passed.

The actual feedback path uses the stated C1-smooth entropy gate, and the
near-gate no-pop regression passed. Changing basis weights, width, entropy
thresholds, feedback gain, or clamp ceilings changes the live codec and remains
Tier 5.

Evidence: `codec/projection.rs`, `codec/structural_evidence.rs`, and
`codec/tests.rs`.
