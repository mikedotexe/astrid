# MLX qualification admission and stop review — 2026-10-21

Historical decision: the [October 22 policy reset](2026-10-22-mlx-local-policy-reset.md)
later superseded the automatic pressure/swap, comparative-percentage and fixed
soak rules described below. Preserve the original observations and failures;
use that later policy and the integration closeout for current instructions.

This record changes the **future MLX package-qualification workflow**, not the
historical result or the reservoir numerical-maintenance gate. The user asked
to stop spending time and API credits on an invented quiet-host prerequisite
while preserving evidence that actually measures a candidate. The standing rule
is in [the runbook](../astrid-mlx-upgrade-runbook.md#offline-and-matched-qualification).

## What the 0.32.4 attempt showed

- The one-hour normal-load passive observation passed: 3,600.387 seconds, 121
  healthy samples, 38 generations, zero fallback/artifacts and 120 stable file
  identities. This qualified observation only, not package performance. Local
  receipt: `.runtime/mlx-upgrade/observations-post-restoration-01/20261021T184317Z/soak_result.json`,
  SHA-256 `aad3884591a6200ba9b417acd4fc06cde051e3ff1409df16ddbf1509492aefd5`.
- A subsequent **read-only** 610-second host window failed the former 300-second
  no-swap rule, even though protected identities and readiness held at every
  sample. It ran no candidate. Local receipt:
  `.runtime/mlx-upgrade/post-restoration-baseline-01/host-window-01/result-01.json`,
  SHA-256 `63a39196f64303bebab52e4dafcf4f2000a3b15a070ddd7b1802a9a4e92aed59`.
  That is a preserved failed *preliminary* check, not evidence of an MLX regression.
- Under a user-approved bounded service hold and exact restoration, both current
  baseline and core passed 15/15 matched workload outputs and the mandatory
  in-batch whole-host memory gate. Controlled-load results cannot establish normal
  production capacity. The final protected closeout is
  `.runtime/mlx-upgrade/post-restoration-baseline-01/controlled-core-window-05/root-final-review-01.json`,
  SHA-256 `11fbfaa5c2768d88ff2f17d3dc55b854103b3408be5c95e22c5332b4e43d6d62`.
- The reversed-order matched comparison still failed performance in four of five
  workloads. For example, exact-output p95 total time was 3.04 times baseline and
  its steady decode throughput was 0.167 times baseline; extended-writing p95
  total time was 1.75 times baseline. The completed comparison is
  `.runtime/mlx-upgrade/post-restoration-baseline-01/controlled-core-window-05/matched-comparison-01.json`,
  SHA-256 `4c07b62266499969b6e916c94561f8398bd8bc802d932b15aefda19327abd`.
  The initial performance failure and reversed-order failure remain failed.
  No qualification proof, activation or MLX promotion was produced.

## Standing decision for the next upgrade

The mandatory `scripts/astrid_mlx_memory.py` gate samples pressure and swap from
immediately before each workload through its drain. It refuses elevated pressure
at start, missing samples, sustained added pressure/swap and retained swap growth.
That gate remains in force. Its result is whole-host evidence and does not by
itself attribute a change to MLX; diagnose other requesters before claiming a
package regression.

The former **separate** five-minute pressure-1/no-swap-growth window, repeated
between environments, is removed as an MLX work-admission prerequisite. It was
an operator rule in the runbook, not an enforced requirement in the package
qualification scripts. Do not reintroduce it through a private plan or confuse
it with the reservoir numeric-case supervisor. Current pressure and competing
build/model load still need a fresh preflight; the workload monitor makes the
final memory decision. If production load prevents an interpretable comparison,
use an explicitly scoped supervised hold with restore verification, as in this
attempt, and label the result controlled-load only. Never force-idle protected
services or treat a brief idle gap as permission for an open-ended experiment.

Plan finite attempts before starting: one matched pair, and only if performance
alone fails, one reversed-order pair. Preserve both receipts. If the repeat still
fails or attribution remains unexplained, stop promotion for that version. Do
offline diagnosis or select a later package candidate; do not erase the result by
lowering performance limits or spending further API resets on periodic memory
checks. Pause unattended qualification follow-ups on that stop. A later MLX
version starts with new frozen manifests, observation and matched receipts; none
of these historical receipts can be carried forward as acceptance.

The user-requested one-hour *routine* core-target baseline is distinct from the
post-activation two-hour soak and Stage 2's accepted core predecessor. The shared
one-hour policy v6 enforces that distinction; it did not waive performance or
in-batch memory gates. Perception's deferred camera/sustained-memory work and the
separate reservoir supervisor retain their own authority and evidence.
