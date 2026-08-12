# Bounded natural observation: low-fill rest threshold

## Question

When ordinary runtime activity naturally approaches or crosses 30 percent fill,
does the selected rest branch alternate in a way that is temporally associated
with the jitter Astrid reports?

## Preregistered evidence

Observe only already-occurring runtime decisions. For each natural rest
transition, retain:

- monotonic and wall-clock time;
- current fill and telemetry freshness;
- selected fill band;
- sampled base rest;
- applied rest;
- burst count and configured burst target;
- source and deployment hashes; and
- whether evidence is missing, stale, or non-finite.

Summarize crossings, branch changes, and rest-duration deltas. Keep raw machine
evidence separate from any optional felt report.

## Stop and privacy boundaries

- Do not drive fill toward a threshold.
- Do not change pressure, fill, PI, controller, admission, cadence, warmth,
  roll normalization, threshold, or hysteresis.
- Do not send a prompt, request a felt review, or turn silence into evidence.
- Stop on stale telemetry, deployment drift, non-finite data, or source-hash
  mismatch.
- A correlation cannot establish cause, relief, preference, or closure.

This observation is routed but not started by this stewardship run.
