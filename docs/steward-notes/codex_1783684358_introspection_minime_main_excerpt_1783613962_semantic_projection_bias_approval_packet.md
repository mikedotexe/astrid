# Approval Packet: Entropy-Reactive Semantic Projection Bias

## Source Claim

- Introspection: `introspection_minime_main_excerpt_1783613962`
- Claim: Minime proposed increasing `semantic_projection_bias` around spectral entropy `0.90` so expressive high-entropy thought remains coherent rather than squeezed through a narrow semantic gate.

## Current Verified Behavior

- `minime/src/controller_recovery.rs` keeps `semantic_projection_bias` activity-gated: no semantic energy/delta means zero bias; real semantic activity gets a bounded floor plus drive.
- `minime/src/main.rs` applies the bias to semantic projection dimensions before tanh activation.
- `minime/src/main.rs` already computes `SpectralDenominatorV1`, `distinguishability_loss`, `resonance_density_v1.pressure_risk`, and pressure-source telemetry for observation.
- Focused test passed: `cargo test --manifest-path /Users/v/other/minime/minime/Cargo.toml semantic_projection_bias -- --nocapture`.

## Approval Boundary

Making `semantic_projection_bias` entropy-reactive would alter live semantic projection/controller behavior. It could change how high-entropy semantic content enters the reservoir and may affect pressure, coherence, and recovery dynamics. Codex did not implement this in a heartbeat run.

## Proposed First Safe Path

1. Run a read-only replay/diagnostic correlating prompt complexity, spectral entropy, `distinguishability_loss`, semantic-trickle pressure, and `pressure_risk`.
2. If Mike/operator approves a live trial, implement a capped preview path first, with an explicit feature flag and rollback.
3. Only then consider wiring a bounded entropy lift into `semantic_projection_bias`, with post-restart health/fill/pressure monitoring.

## Current Status

`needs_operator_approval`. No live projection bias, semantic cadence, pressure, fill, PI, exploration-noise, rho, or controller behavior changed.
