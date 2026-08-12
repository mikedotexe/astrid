# Dynamic Noise Pressure-Room Replay

## Question

Across already-recorded Minime telemetry, does the source-prepared dynamic-noise
candidate saturate outside its pressure window often enough to erase useful
variation, and how does that compare with gradient, entropy, and fill?

## Frozen Inputs

- Exact Minime `minime/src/esn.rs` SHA-256 at execution.
- Existing immutable telemetry or diagnostic records only.
- `calculate_dynamic_noise` reproduced from the source-bound implementation.
- No generated live sensory input and no connection to ports 7878-7883.

## Outputs

- Coverage and missingness by field.
- Pressure-window occupancy and candidate-output distribution.
- Candidate saturation by gradient, entropy, and fill bins.
- Source hash, record hashes, deterministic manifest, and rerun digest.

## Stop And Authority Boundaries

Stop on source drift, malformed or non-finite records, insufficient joined
coverage, or any attempted live socket access. Association is not felt cause,
benefit, or permission to alter exploration noise, pressure, rho, PI, fill,
cadence, or reservoir behavior.
