# Minime ESN, Sensory-Bus, and Regulator Review - 2026-07-10

Run: `codex_1783687024`

Sources fully read:
- `introspection_minime_esn_1783686851`
- `introspection_minime_sensory_bus_1783686388`
- `introspection_minime_regulator_1783685980`

## Implemented Or Verified

- ESN high-entropy/noise stutter: verified existing read-only source and tests in `/Users/v/other/minime/minime/src/esn.rs`. The reported `entropy=0.90`, gentle gradient, pressure-room, and foothold case is covered by `exploration_noise_coherence_review_v1`; entropy-ceiling/noise-damping authority is covered by `entropy_ceiling_noise_damping_review_v1`.
- Sensory-bus stale-window continuity: verified existing tests in `/Users/v/other/minime/minime/src/sensory_bus.rs` for bounded recovery handover, 0.35-0.45 release, and soft-knee surge tapering.
- Regulator sticky-flow diagnosis: verified existing tests in `/Users/v/other/minime/minime/src/regulator.rs` for viscosity/cohesion/flow separation, effective mobility, pressure-source visibility, pressure-porosity gradient, and high-mode-packing observational behavior.

## Sandbox Observation

- Post-promotion queue refresh created runner-ready `trial_08d1e26ca50d6cb3` for the ESN dispersal-potential claim. Codex ran it with `shadow_loss_lattice_v1`; the read-only result classified current evidence as `lattice_transition_like` with 20 samples, 47 lattice-language hits, 0 loss-language hits, min norm delta `-0.006`, and max dispersal `0.25`. Result JSON/Markdown and a right-to-ignore result card were written under `capsules/spectral-bridge/workspace/diagnostics/sandbox_trial_queue_v1/`.

## Authority Boundaries

- Live exploration-noise injection at `0.13`, lowering active dynamic-noise gradient thresholds, or raising the volatile entropy ceiling would change Minime ESN/runtime behavior.
- Retuning sensory stale-window sigmoid steepness or binding it to operational velocity would change live semantic persistence and sensory cadence.
- Changing regulator flow/mobility formulae or adding porosity-based pressure compensation would alter live pressure/regulator behavior.

No live pressure, fill, PI, sensory cadence, controller behavior, exploration noise, pressure thresholds, stale-window cadence, regulator formula, Minime process, bridge process, staging, git add, commit, or push was performed by Codex.
