# MLX patch-upgrade policy reset — October 22, 2026

The operator directed completion of the 0.32.3 package upgrade and an overhaul
of locally invented gates that repeatedly delayed this one-time transition.
This decision applies prospectively to the remaining Stage 2 qualification and
future patch-version intakes. It does not change any historical receipt. In
particular, the first Stage 2 offline result and matched smoke remain failed.

## What decides acceptance

The hard checks are the selected package/source/model identity, loader and
generation behavior, all required output contracts and sampled writing quality,
request-specific completion deadlines, live provider/reservoir readiness, no
crash/OOM/unexpected fallback, clean candidate exit, and an exact retained
rollback path. A real service or output failure stops promotion. A new receipt
must show these checks under the prospective policy before a live handoff.

The following measurements remain recorded but **do not automatically reject a
patch upgrade**: absolute or growing swap, macOS memory-pressure level or a
brief pressure spike, percentage changes in p95 latency/throughput/peak MLX
allocation, and the 1.5 ms/token speed limit of a wide-coupling path disabled
in both selected launch configurations. If pressure causes an OOM, a candidate
failure, or a production health failure, that observed failure is a hard stop.
If active requests exceed their own deadlines or sampled quality fails, that
is also a hard stop. Review user-visible timing and disclose regressions; do
not turn an arbitrary ratio into a veto when service behavior remains useful.

There is no mandatory quiet-host window, historical-swap ceiling, repeat
one-hour baseline after unchanged identities, or fixed-duration postactivation
soak as a prerequisite for the next patch stage. Run one bounded, same-window
comparison and live service check. Continue passive monitoring without holding
up independent source preparation. A completed observation may strengthen the
case, but elapsed time alone is neither proof nor a reason to consume more
credit. Do not initiate repeated attempts under unchanged conditions.

## Applying this decision to the current rollout

The scripts and proof reader were updated together and qualified with model-free
fault tests, fresh offline core/full work and one matched pair. Accepted proof
is `.runtime/mlx-upgrade/stage2-0323-01/policy-reset-integration-01/full-proof-01.json`.
Full MLX/Metal 0.32.3, MLX-LM 0.32.0 and Transformers 5.7.0 activated under
transaction `256b7a3f2f464d77bb8d4d9eb5cb47ba`; exact live output, readiness,
reservoir connection and selected versions passed. The accepted core-only
release is retained as immediate rollback. Earlier failed receipts remain failed.

Both observers have finished. The core observer crossed the approved Stage 2
transition, recording failed uninterrupted-identity checks and one cutover
fallback; it is not an accepted uninterrupted core soak. The perception
observer recorded 121 samples over two hours under one process identity, with
physical footprint 41.45-51.81 MB and fresh visual/ASCII/audio records. This is
bounded footprint evidence, not allocator causality or lifetime leak freedom.
The exact closeout is `.runtime/mlx-upgrade/stage2-0323-01/observer-closeout-01.json`.
Neither observation authorizes another timer or replay of a spent transition.

For future patch upgrades, keep the above distinction in the runbook and
qualification code. Do not recreate a new quiet interval, swap threshold,
percentage ratio, or long soak as an implicit prerequisite without a measured
user-facing failure that it detects. One final integrated review is preferable
to serial reviews of every source-only draft. Keep the scheduler quiet on
unchanged observations and spend runs on finite work, failures, or decisions.
