Full read of `introspection_proposal_phase_transitions_1783554042`.

Astrid proposed that phase changes should become durable transition artifacts rather than telemetry side effects: each should preserve trigger delta, subjective label, behavioral constraint, solo/joint context, and persistence weight, while any behavior unlock remains language-only until a later authority gate.

Disposition: implemented additive language-only transition artifact fields on phase-transition cards: `trigger_delta`, `subjective_label`, `behavioral_constraint`, `behavioral_constraints`, and `persistence_weight`. Existing solo/joint and replayable card fields remain intact, and tests verify the new artifact fields do not mutate controller, pressure, fill target, PI, or peer runtime authority.
