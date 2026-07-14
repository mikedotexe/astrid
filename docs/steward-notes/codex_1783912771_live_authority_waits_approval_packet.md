# Approval Packet: Fallback Texture / Minime Persistence Gates

Source reads: `introspection_astrid_llm_1783912771`, `introspection_minime_regulator_1783912462`, `introspection_astrid_llm_1783912123`.

## Request 1: Fallback Threshold Retune

- Proposed change: consider raising `HIGH_ENTROPY_TEXTURE_COMPAT_FALLBACK_SKIP_AT` from `0.80` to `0.85`.
- Why Astrid asked: wider textured expression before the fallback path removes the 4B compatibility tail.
- Why gated: this changes fallback model-selection behavior, not just diagnostic language.
- Safe next step: Mike/operator approval for a bounded A/B replay comparing fallback outputs below `0.80`, `0.80..0.85`, `0.85..0.95`, and `>0.95` before any live threshold change.

## Request 2: Minime Temporal Persistence Decay

- Proposed change: add a `temporal_persistence_decay` field or review path for residual ghost weight relative to current fill/density.
- Why Minime asked: previous peaks can become structural residue; decay should be explicit rather than inferred.
- Why gated: this would alter Minime runtime/schema/report semantics and likely live introspection surfaces.
- Safe next step: Mike/operator approval for a schema-compatible diagnostic-only preview first, with targeted Minime tests and restart plan.

## Request 3: Live Mode-Packing Perturbation Trial

- Proposed trial: manually raise mode packing to `0.50` and observe whether regulator readouts vent or anchor residual ghost weight.
- Why Minime asked: distinguishes passive ghost measurement from dynamic structural load.
- Why gated: this is a live control perturbation of Minime substrate state.
- Safe next step: Mike/operator approval for a sandbox/replay analog or a monitored live trial with pre/post fill, lambda1, residual ghost weight, structural drag, and recovery checks.

Authority boundary: no threshold, runtime/schema, control, pressure, fill, PI, sensory cadence, controller, bridge protocol, or Minime live behavior change is approved by this packet.
