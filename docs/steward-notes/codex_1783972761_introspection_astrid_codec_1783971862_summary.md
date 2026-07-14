Full read of `introspection_astrid_codec_1783971862.txt`.

Astrid reports that `codec.rs` is the deterministic text-to-48D semantic bridge, including fixed dimensional ranges, the fixed-seed 768D-to-8D embedding projection, tail-vibrancy smoothstep gating, and projection epoch persistence. The report names two risks: runtime path fallback can silently point at the wrong location if `ASTRID_CODEC_RUNTIME_DIR` is unset, and floating-point determinism could affect projection checksums if the matrix is recalculated. Suggested validation focuses on monitored overflow dimensions and the follow-up hook/clamp boundary.

Disposition: verified existing source/tests rather than changing live vector width, gain, ceiling, runtime path policy, projection algorithm, or bridge protocol.
