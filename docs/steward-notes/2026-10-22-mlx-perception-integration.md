# MLX And Perception Integration

## Authority And Scope

Mike assigned this chat the remaining integration after the MLX owner finished.
The owner explicitly handed off shared Git/edit ownership and confirmed no
additional MLX or perception deployment was required. The read-only handoff is
`.runtime/mlx-upgrade/stage2-0323-01/final-integration-handoff-01.json`, SHA-256
`f2972fcd4f56f765a71a2f196658e849521dd305506dbe006a59ad4ed6b881b4`.
The parent is the sole Git coordinator. Review agents own only their named files.
Automations remain paused at controller generation 534; no automation round,
canonical-report completion, model experiment or new service transition is claimed.

The pre-integration Astrid main and remote were
`b1d552f2f4c6b330978f10c79eda41fe25328c68`; Minime remained clean at
`73f1e390565c52f83c77fd4ffd31d38103c3bb33`. Larger-capacity work, frozen
navigation/exploration releases, private state and rollback assets are excluded.

## Existing Live Release

- Full MLX/Metal 0.32.3, MLX-LM 0.32.0 and Transformers 5.7.0 were already
  `activated_verified` under transaction `256b7a3f2f464d77bb8d4d9eb5cb47ba`.
  Provider PID 77735 started October 22 at 11:51:09 local time.
- Activation receipt SHA-256:
  `3e13f4dd38d909b178a81cd7f3def11c0fef6051b71f94be8accc522d6cbc954`.
  Selected manifest SHA-256:
  `42936d2ba3d0f90bbf2903ee14380fab2004723a4052799e2fff1c5a5389c98a`.
- Accepted Stage 2 proof SHA-256:
  `e91b3002e6bc875695bd7583bc13c14af93834f6c4a01ab0620a2dd35b8b6f3f`.
  Live exact-output smoke SHA-256:
  `dcbecdae5704ce0dd372a28030df4b14b9d59d1e46d93e8cc64ffdf68d70d1ca`.
- The immediate core-only 0.32.3 and original 0.32.0 rollback assets remain.
  The unsuccessful 0.32.4 trial is not relabelled accepted.
- Perception PID 53608 started October 22 at 10:25:57 local time. Its reviewed
  `perception.py` SHA-256 is
  `d842d08aa505836451c64e0ad29158d7610efc6828e54a2988ccfe8f28c648a8`;
  `camera_capture.py` is
  `4692f30fe56090490cc648b78447e993a368a5a148ef53a0b2e38d235caae34d`.

## Completed Observations

The final observer closeout is
`.runtime/mlx-upgrade/stage2-0323-01/observer-closeout-01.json`, SHA-256
`13b1991e2e61585cf124ded6a93a1a89f7f30ef63c38445716b83cf7641eccbb`.

The old core observer recorded 240 samples over 7,200 seconds, crossing the
approved Stage 2 transition. It failed uninterrupted-identity/readiness checks
and recorded one bridge fallback at 18:51:31.585276 UTC. Retain it as interrupted
observation, not a clean core soak or a new failure of the later Stage 2 proof.
No repeat waiting period or model workload is implied.

Perception recorded 121 samples over 7,200.024 seconds under one identity.
Physical footprint ranged from 41,452,120 to 51,806,808 bytes; the first-ten
median was 46,121,560 and the last-ten median 49,005,144. The observation saw
113 fresh visual, 136 ASCII and 20 audio records. This supports bounded
post-containment operation, not allocator causality or lifetime leak freedom.

## Integration Review

The runbook and current policy note now distinguish the accepted patch policy
from the old mandatory-soak instructions. Historical receipts and source
snapshots retain their original outcomes. The perception closeout now records
the completed hardware and two-hour checks, rather than leaving them pending.

Model-free review found a separate stack-receipt problem: failed model identity
resolution could abort the shell wrapper before it wrote a failed diagnostic.
The focused repair keeps failed identity checks fatal to acceptance while
allowing a truthful failed receipt, with no guessed interpreter fallback.

Two additional synthetic findings concern launch tooling, not an observed live
failure: exclusive-create collision cleanup must not delete a temporary file
it did not create, and an added `.pth` startup hook can escape the declared
package/artifact identity. The temporary-file ownership repair is included.

The proposed startup-hook rejection was tested but is deliberately not shipped:
the full/core environments' `distutils-precedence.pth` is covered by setuptools'
hashed RECORD, but the retained original rollback has a generated
`_virtualenv.pth` without that coverage or an explicit artifact binding. A new
rejection would disable the existing rollback route. Preserve that candidate
and its tests separately; the next bounded repair is a reviewed binding of the
legacy startup closure and compatibility qualification before enabling rejection.
No allowlist, historical-manifest rewrite, rollback removal or weakened check
is introduced to conceal this limitation. Current package validation is not a
claim of fully hermetic Python startup.

The historical offline-reuse test incorrectly expected an old compatibility
exception to accept the current, changed harness. The corrected test separately
checks the frozen exception and current refusal. No compatibility-map hash,
historical receipt, accepted proof or guard was relaxed to turn that test green.

## Reservoir Repository

The MLX owner also confirmed two already-live changes in
`/Users/v/other/neural-triple-reservoir`, initially at
`e719cd7f06af1c63fa3288c9689e479a47503586` on its existing tracked branch
`feat/service-stack-and-multi-headed`. No branch rename or unrelated merge is
implied. The review packet is
`.runtime/mlx-upgrade/stage2-0323-01/reservoir-dirty-review-01.json`, SHA-256
`34b314cb926922ebacacf25dc5ada86ad4990677a990aa11ca8aad87a6c0a363`.

- `reservoir_service.py` matches the accepted production42 frozen source:
  `cd887e536de210592f6f719ffaff16be5508869b48b3594fc53956504968b9da`.
  Transaction `662bdd2c72fc11fe4b2d703bd64cc796` was already accepted; its review
  SHA-256 is `ab4e1a6ba43f50951536379854769d6dcd3b0c61a041ff3cdefe6b4653c7668f`.
  Reservoir PID 98078 has run since October 20 at 20:45:55 local time.
- The repository and installed `com.reservoir.coupled-astrid.plist` match:
  `cfade671f87ec1f633aea31ac86fd8c513faf1435cd8d7be78484e760e79399b`.
  This is machine-local launch configuration. Its Python 3.14 stdlib launcher
  selects the manifest's Python 3.12 provider; it does not run MLX on Python 3.14.
- Periodic saves yield between captured handles, while each save and shutdown
  batch remain synchronous. One slow handle can still block. No atomic batch,
  resident-memory equivalence or absence of latency spikes is claimed.
- Integration retains those exact production bytes, adds model-free synthetic
  regression tests and records the existing deployment. No reinstall, restart,
  reservoir mutation or maintenance replay is performed.

The two evidence directories are retained as explicitly inventoried historical
material, separate from current executable source. The October 15 inbox-format
receipt is a separate content-free historical record; retaining it does not
repeat conversion, send a letter or establish delivery or understanding.

## Qualification And Closeout

The final model-free integration run passed **350 tests and 78 subtests** in
14.96 seconds across 20 explicit modules. Exact commands, tested source hashes,
JUnit and output are retained under
`.runtime/git-integration/mlx-perception-20261022/final-pytest-rollback-preserved.*`.
Source hashes were unchanged through the run. The earlier 350-pass/one-failure
review remains in `mlx-toolkit-review.yZYTZS/focused.log` beneath the retained
`freer-exploration-20261021` worktree; the failure was the stale historical
compatibility expectation described above, not an observed live failure.

The independent perception/identity review passed 57 unit tests and all eight
first-SIGINT scenarios, with 18 owned containment children reaped. These suites
overlap the aggregate tooling run and are not added into its test count. The
fake-camera tests reconstructed the production bytes in
`/private/tmp/astrid-perception-review.4S8xKp`, without camera/model access.
The new real-shell wrapper tests cover absent PID, stale context, resolver
failure, malformed/partial output and verified identities. Three existing
deployment-wrapper checks also passed. Bash syntax and source diff checks pass.

The reservoir's new stdlib-only AST-extracted suite passed **11 tests** under
`python3 -I -B -W error test_cooperative_autosnapshot.py`. It exercises the actual
selected methods without importing the service, opening sockets or models.
LaunchAgent plist syntax passed; repository and installed bytes remain equal.

The separately retained startup-hook candidate is in
`/Users/v/other/worktrees/freer-exploration-20261021/model-environment-fixes.awaaRm/deferred-pth-candidate-01/`.
Its patch SHA-256 is
`ef9211962166634f0f24eb4c7e0bfe8b413de92a8b9088ce7b14120ab3666b8b`.
Its 179-test/50-subtest result is not release acceptance: compatibility with
the original rollback failed, so those gate changes were removed from the
canonical candidate. The final suite above tests the rollback-preserving form.

Some qualification tests intentionally use retained local `.runtime` receipts;
the real-receipt offline test now skips explicitly when those are absent, and
source-only portability of the remaining performance-evidence test remains debt.
No production model qualification was repeated. Some existing large tooling
modules remain cohesive, centralized transition/proof readers; this integration
does not refactor their ownership or alter frozen release inventories.

No Rust build or new model/camera workload was performed. No build caches,
rollback assets or authored state were deleted; available disk space was about
398 GiB. Final read-only health showed the provider ready and reservoir-connected;
provider, bridge, agent, engine, perception, Division and sensory PID/start
identities were unchanged. Source tests do not substitute for existing activation
receipts or establish subjective improvement. Git results are recorded below.
