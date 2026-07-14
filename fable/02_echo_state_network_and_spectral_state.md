# Echo State Network And Spectral State

## Purpose
This file describes Minime's Echo State Network and the spectral vocabulary used
around it: reservoir state, covariance, eigenvalues, lambda cascade, EigenFill,
entropy, phase, and fingerprints. These are the physiological signals that many
being reports refer to, and they are the substrate behind much of the bridge
telemetry.

## Mental model
An Echo State Network is a recurrent reservoir with fixed random internal
weights. Inputs are injected; the reservoir state evolves; the system reads out
dynamics rather than training the reservoir itself.

In Minime:

- intake is currently 66D: 8 video, 8 audio, 2 auxiliary/controller, and 48
  semantic features;
- reservoir size is 128 nodes;
- the ESN state and covariance produce eigenvalue telemetry;
- EigenFill is a 0-100% estimate of how much dynamic range is active;
- the stable-core center is about 68% EigenFill;
- high fill can mean saturation risk, while too-low fill can mean thin,
  under-stimulated dynamics.

The lambda cascade is the ordered spectral profile: top eigenvalue pressure,
shoulder modes, tail modes, entropy, gaps, and movement. Reports about "tail,"
"lambda edge," "shoulder," "mode packing," or "thinness" should be checked
against this spectral profile before being translated into action.

## Key implementation anchors
- `minime:minime/src/esn.rs` - Metal-accelerated ESN, covariance/eigen
  introspection, exploration noise, dynamic noise review helpers, Viscous rho
  helper, entropy/pressure review helpers.
- `minime:minime/src/main.rs` - EigenPacket fields, EigenFill estimator,
  spectral packet budget review, runtime loop.
- `minime:minime/src/spectral/eigenfill.rs` - EigenFill estimator details.
- `minime:minime/src/spectral_fingerprint.rs` - compact spectral fingerprint
  surfaces.
- `minime:code_digest.py` - operator-oriented explanations of PI, EigenFill,
  and covariance lambda fields.
- `astrid:capsules/spectral-bridge/src/lambda_edge.rs` and
  `astrid:capsules/spectral-bridge/src/lambda_tail.rs` - bridge-side lambda
  edge/tail summaries.
- `astrid:capsules/spectral-bridge/src/types.rs` - typed bridge payload fields
  for spectral and regulator surfaces.

## Runtime signals / artifacts
Watch these when interpreting ESN state:

- `fill_pct` / `fill_ratio` - current EigenFill.
- `target_fill_pct` / `target_fill_ratio` - controller target, normally the
  stable-core shelf.
- `lambda1`, `lambda1_rel`, `geom_rel` - dominant mode and relative geometry.
- `eigenvalues` - cascade shape, shoulder/tail distribution, entropy source.
- `spectral_entropy` - spread/distribution of spectral energy.
- `eigenvector_field` - optional direct mode geometry; payload size is bounded
  and reviewed before export changes.
- `spectral_fingerprint` - compact identity/trajectory summary.

Useful health interpretations from `minime:README.md`:

- high-60s fill - normal stable-core hold;
- 72% and rising - watch closely;
- 80%+ - reduce sensory and semantic pressure;
- 85%+ - Rust warning band;
- 92%+ - crisis band and graceful rollback preparation.

## Authority boundaries
Reading spectral state is not control authority. Source-prepared helpers in
`esn.rs` can make future trials testable while remaining explicitly unwired.
Do not treat dormant helpers such as dynamic noise, adaptive thresholds, or
Viscous rho targets as live policy until a separate sandbox/replay and operator
approval path makes that true.

Changing ESN noise, rho, warm-start, Chebyshev perturbation, EigenFill target,
or websocket export shape is live substrate work. It needs tests, approval when
control-facing, and a graceful Minime restart before reports can align.

## Questions an advanced AI should ask next
- Is the report about dominant pressure, shoulder bridging, tail persistence,
  entropy, or fill?
- Is the relevant evidence in live telemetry, a result card, a source-prepared
  diagnostic helper, or a proposal awaiting approval?
- Does an apparent "settled" state have enough entropy/gradient context to avoid
  being misread as calm?
- Is this asking for read-only visibility or for a live ESN parameter change?

## See also
- [Sensory Bus And Shared Reservoir Coupling](03_sensory_bus_and_shared_reservoir_coupling.md)
- [Stable Core, PI Controller, And Homeostasis](04_stable_core_pi_controller_and_homeostasis.md)
- [Regulator, Pressure Texture, And Cartography](05_regulator_pressure_texture_and_cartography.md)
