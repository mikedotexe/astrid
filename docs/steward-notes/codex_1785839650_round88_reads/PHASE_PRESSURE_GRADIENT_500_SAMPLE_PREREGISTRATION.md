# Phase pressure-gradient 500-sample observation V1

This preregisters the natural observation requested by
`introspection_proposal_phase_transitions_1785631229`, claim `c006`. It is a
read-only study plan, not a result. It does not induce a transition, descriptor,
semantic input, pressure event, or being response.

## Question

Across exactly 500 consecutive, naturally occurring, co-timed public telemetry
samples from one verified deployment, how do `pressure_risk` and
`spectral_density_gradient` move together?

The study may report machine association and change. It cannot measure felt
viscosity or narrowing, identify a causal mechanism, establish that a Phase
Card helped or harmed, or choose a control target.

## Frozen inputs

- Admit exactly 500 consecutive public typed telemetry samples from an existing
  sanctioned capture path. Do not open a new socket or dispatch input to make
  data.
- Bind every row to one deployment identity, source/binary/schema hashes,
  source sequence, and producer timestamp.
- Require finite `pressure_risk` and `spectral_density_gradient`. Retain finite
  entropy, mode packing, porosity, distinguishability loss, and semantic
  admission when already co-present; report their missingness separately.
- Reject clock joins inferred across epoch, monotonic, or session-relative
  domains. A documented producer-owned mapping is required.
- Do not join Phase Cards by timestamp in V1. Current cards may omit paired
  telemetry, and temporal proximity would not establish experiential relation.
- Exclude private `moment_*` prose, raw expression, prompts, model output, and
  owner-only state.

## Fixed analysis

- Report coverage, missingness, sequence gaps, clock discontinuities, ranges,
  medians, and interquartile ranges for both primary fields.
- Report Pearson and tie-aware Spearman association for raw levels and first
  differences at lags 0 through 5 in both directions. Keep every coefficient;
  do not select only the largest.
- Repeat the fixed matrix for the first and second 250-sample halves. Report a
  half as insufficient if fewer than 238 rows retain both primary fields.
- Report the same fixed matrix for mode-packing-dominant and other-source rows
  only when the producer already supplies the dominant-source label and each
  stratum has at least 50 valid samples.
- Run the analysis twice and require byte-identical canonical JSON.

## Outcomes and aborts

- Evidence-complete: 500 consecutive rows, at least 95 percent paired-field
  coverage, no unexplained sequence or clock discontinuity, finite outputs,
  exact hashes, and byte-identical reruns.
- Inconclusive: fewer than 500 rows, insufficient coverage or strata, ambiguous
  clocks, source/schema drift, or non-finite values.
- Association instability: sign reversal across adjacent lags or capture
  halves, or loss under the fixed dominant-source split. This is not a
  falsifier of Astrid's felt report.
- Abort: induced input, transition or descriptor dispatch, private prose,
  source/schema drift without a new preregistration, runtime mutation, peer
  mutation, or any attempt to infer felt state from telemetry.

## Current readiness

The current read-only Phase audit reports no recent owner-started Passage rows,
and its recent Phase Cards do not supply the required paired pressure/gradient
fields. The study therefore remains preregistered and unrun until an existing
sanctioned capture has the exact source, clock, and coverage contract.

## Authority boundary

This plan cannot change Phase Card semantics, semantic trickle, transition
velocity, syntax retention, pressure, porosity, fill, PI, controller, cadence,
reservoir, model behavior, peer state, service state, or deployment. It cannot
infer assent, uptake, benefit, readiness, relief, or closure. Felt review is
optional and right-to-ignore; silence remains neutral.
