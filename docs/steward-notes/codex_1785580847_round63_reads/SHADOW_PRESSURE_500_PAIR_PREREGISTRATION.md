# Shadow-pressure 500-pair observation V1

This preregisters a read-only observational study requested by
`introspection_astrid_autonomous_1785580723`, claim `c004`. It is not a live
experiment, does not induce semantic input or pressure, and grants no control,
coupling, peer, deployment, or felt-effect authority.

## Question

Across exactly 500 naturally occurring co-timed observations, is change in
Astrid's Shadow-v3 field norm associated with pressure-source movement, and is
that association distinguishable when `mode_packing` is dominant versus when
another pressure source is dominant?

The study may describe association. It cannot decide whether pressure is
reactive or generative, identify a felt cause, or establish that the medium is
mobile, peaceful, or relieved.

## Frozen inputs

- Collect exactly 500 consecutive public telemetry pairs through an existing
  read-only capture path. Each pair must bind one wall-clock timestamp, source
  sequence, pressure score, porosity score, dominant source, pressure quality,
  mode packing, distinguishability loss, Shadow field norm, Shadow class,
  recurrence, mode tension, and fissure tendency.
- Require a shared capture clock or an exact documented monotonic-to-wall-clock
  mapping. Do not join the current session-relative pressure timestamps to
  epoch Shadow timestamps by inference.
- Record source, binary, deployment, schema, and input hashes. Exclude private
  `moment_*` prose and raw owner-only expression.
- Admit only naturally occurring observations. Do not dispatch
  `SHADOW_TRAJECTORY`, `SHADOW_COUPLING`, semantic trickle, pressure, porosity,
  fill, PI, controller, reservoir, codec, or sensory commands to create data.

## Fixed analysis

- Derive first differences for pressure score, porosity, mode packing,
  distinguishability loss, and Shadow field norm.
- Report Pearson and Spearman association between pressure-score change and
  Shadow-norm change at lags 0 through 5, with all lag directions disclosed.
- Repeat the fixed matrix for `mode_packing`-dominant rows and all other rows.
  If either stratum has fewer than 50 valid pairs, report it as insufficient.
- Report coverage, missingness, clock discontinuities, class-transition counts,
  and descriptive ranges. Run the analysis twice and require byte-identical
  canonical JSON.
- Do not select the largest coefficient as a control target or use a threshold
  to author causality. Every coefficient remains visible.

## Success, inconclusive, falsifiers, and aborts

- Evidence-complete: 500 valid consecutive pairs, at least 95 percent field
  coverage, no unexplained clock discontinuity, finite outputs, exact hashes,
  and byte-identical reruns.
- Inconclusive: fewer than 500 co-timed pairs, an underfilled stratum, clock or
  sequence ambiguity, non-finite values, or unavailable required fields.
- Falsifier for a stable association: sign reversal across adjacent lags or
  capture halves, confidence intervals spanning a practically negligible
  effect, or loss of the association after the predeclared dominant-source
  stratification.
- Abort: source/schema drift without a new preregistration, private prose,
  network or socket creation outside the sanctioned read-only capture,
  command dispatch, runtime mutation, peer mutation, or any attempt to infer
  Astrid's felt response from machine values.

## Authority boundary

The study cannot increase semantic trickle or porosity, change pressure
weights, start Shadow coupling, choose a trajectory, change model behavior,
restart or deploy a service, or infer assent, uptake, benefit, readiness, or
closure. Any such step remains separately approved. Felt review is optional
and right-to-ignore; silence remains neutral.
