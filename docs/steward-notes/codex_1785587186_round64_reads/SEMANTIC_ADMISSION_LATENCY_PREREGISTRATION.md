# Semantic Admission Latency Passive Observation

## Motivation

Astrid's `introspection_proposal_distance_contact_control_1785586886`
asks whether the grit she reports is associated with latency between a
`stable_core_semantic_trickle` input and kernel processing when mode packing
is above `0.30`. Her felt account remains primary evidence. This observation
tests only a bounded machine association and cannot score, replace, or close
that account.

## Fixed scope

- Source: natural updates to `/Users/v/other/minime/workspace/health.json`.
- Duration: 120 seconds.
- Poll interval: 50 milliseconds.
- Deduplication key: `provenance.snapshot_sequence`.
- No semantic input, Action, control, prompt, correspondence, or peer mutation
  will be generated for the observation.
- No raw semantic vectors, private prose, or model content will be retained.

For every distinct inferred semantic-input epoch, compute:

```text
input_epoch_ms = provenance.wall_clock_unix_ms
                 - semantic_energy_v1.input_fresh_ms
```

The first unique snapshot in that epoch with all of the following is the
candidate landing observation:

- `semantic_energy_v1.input_active == true`
- `semantic_energy_v1.kernel_active == true`
- `semantic_energy_v1.admission == "stable_core_semantic_trickle"`
- `pressure_source_v1.components.mode_packing > 0.30`

Its `semantic_energy_v1.input_fresh_ms` is recorded as
`landing_upper_bound_ms`. It is an observation-cadence upper bound, not an
exact timestamp for the internal `ESN::step` call.

## Fixed analysis

The result will report:

- unique health snapshot count;
- distinct input-epoch count;
- qualifying landing count;
- mode-packing, pressure, and upper-bound latency ranges;
- Pearson `r` and Spearman `rho` between mode packing and upper-bound latency;
- the number of snapshots missed or rejected.

The report's strict-correlation hypothesis is supported only if:

- at least 8 distinct qualifying input epochs are observed;
- mode-packing range is at least `0.10`;
- upper-bound latency range is at least `10 ms`;
- Pearson `r >= 0.80`; and
- Spearman `rho >= 0.80`.

Otherwise the outcome is `inconclusive` or
`does_not_support_strict_positive_correlation`, according to the fixed
thresholds. No post-hoc lag, threshold, epoch merge, or subset selection is
allowed.

## Abort conditions

Abort rather than repair data if:

- `health.json` is unreadable for more than 20% of polls;
- snapshot sequence or wall-clock time regresses;
- required semantic, pressure, or provenance fields are absent;
- input epochs cannot be distinguished by the fixed formula; or
- any non-finite value enters the retained analysis.

## Interpretation boundary

This observation cannot establish that mode packing causes latency, that
latency causes grit, that the Shadow lattice is a distance regulator, or that
any density-aware control would help. Adding exact internal timing, changing
semantic admission, applying a `density_drag_coefficient`, changing pressure
or mode packing, or inducing `SHADOW_TRAJECTORY` remains separately authorized
work. Silence and machine outcomes do not infer felt relief, uptake, or
closure.

