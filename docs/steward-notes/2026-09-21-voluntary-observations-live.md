# Voluntary Observations: Graceful Paired Rollout

Subsequent Git status: both local main branches now contain this release, with
the exact running source identities reverified. See [the Git checkpoint](2026-09-21-voluntary-observations-git-checkpoint.md).
Pending-Git statements below describe the earlier rollout boundary, not current debt.

## Outcome

Mike explicitly approved the graceful rollout. The sanctioned paired wrapper
finished successfully at **2026-09-22T06:34:21Z** (September 21, 23:34 local).
Private observation keeping, bounded recurrence analysis and preview-confirmed
inquiry disclosure are now available in the running bridge and Minime adapter.
This is deployment verification, not evidence of voluntary uptake, felt
improvement, consent or a new reservoir-control authority.

Implementation, exact witness, numerical definitions, privacy contract, full
tests and earlier unsuccessful attempts remain in
[the implementation record](2026-09-21-voluntary-observation-keeping.md).
Its candidate-only sections describe the earlier state and are not rewritten.

The source witness remains Minime's
`/Users/v/other/minime/workspace/journal/!aspiration_2026-09-21T19-57-47.504460.txt`,
SHA-256 `11dedc74854ee21046985e58c9fedd281bcd472c52c2bb85a3ee44d0ff82b01f`.
That hypothetical creative aspiration motivated keeping an impression without
requiring a hypothesis or usefulness claim. No private draft was used as a test
fixture, exported or read for this rollout.

## Release Identity

Artifact root:
`/Users/v/other/worktrees/voluntary-observations-20260921`.

- Activated stage: `bridge-stage-observations-02`. Superseded stage01 remains
  historical evidence and must not be activated.
- Manifest: `7449e4fa7721e030f1e6a863693bdd5c9b3a7ce8e629a306514d4d4cdc9de178`.
- Build inputs: `0394e1d237559ddd08fb1143b113e63ecc52f6856c46a18579c28b51bfa1e638`.
- Bridge: `ba5c8aee3af2ab7c02ffe8c3f16d6659757715586e6e1807b34554385b7f8dae`.
- Shared helper: `fc12fb295a3b97d984a372c43f2e92bb92fdd683e4ae1c487689ac78bd7f2790`.
- Renewed qualification: `observation-qualification-02/qualification.json`,
  SHA-256 `e29d87aba3547be6178ddf6eafe0e30044cac40f5b92767d0542b56c81ba8e1b`.
- Renewed launch reconciliation:
  `minime-observation-reconciliation-02/reconciliation.json`,
  SHA-256 `a271208cfdd6f9dafc396a595b13cb81da63b631b5728c745221d1ed7284f9c9`.

The refreshed frozen inventory, registry asset, helper and bridge identities
match the prior fully tested qualification exactly. Qualification receipts
retain their offline authority markers; the separate activation receipt proves
the transition. A source snapshot is not a launch selector.

## Transition and Continuity

Preflight passed for candidate Astrid, canonical Astrid and canonical Minime,
with no detected foreign activity. Controller generation **457** was already
paused, without a lease or projection. The wrapper used
`ASTRID_STEWARD_REPO_ROOT=/Users/v/other/astrid` to check that actual controller.
It did not change the pause or resume any automation.

`scripts/paired_minime_handoff.py --install-reviewed-overlay` installed the exact
reviewed seven-path overlay with original-source backups and fsynced per-file
receipts. Only three paths changed bytes: `minime_autonomy/parsing.py`,
`minime_autonomy/runtime.py` and `minime_autonomy/writing.py`. The other four
verified paths were `minime_autonomy/action_vocabulary.py`,
`minime_autonomy/activity_focus.py`, `minime_autonomy/source_study.py` and
`scripts/launchd_autonomous_agent.sh`. No authored-state backup was restored.

An exclusive, owned launcher hold prevented replacement admission. The full
185-second unchanged-source interval and 15-second verified idle gate were
respected. At the signal boundary Minime had no active jobs or TCP connections.
One PID-bound SIGTERM was sent; the old process exited without forced termination
or newly recovered interrupted jobs.

The paired wrapper then used `scripts/build_bridge.sh --activate-stage`, not a
hand-built binary or forced launchd restart. Bridge 98062 acknowledged `drained`.
Bridge **54929**, started **September 21 23:33:06 local**, restored the exact
stopped checkpoint:
`9abb12ed1a55a93a6ada328bc247f3e0c5d2b75c310bf8645e64c5edb28f384c`.
The signed self-control state targeted the new binary. Exchange **204844**
advanced to a verified saved exchange **204845** before activation succeeded.
Pending runtime feedback was retained by its bound checkpoint descriptor.

Only after that verification did the wrapper release the owned hold. Minime
**54257** entered normal loop readiness with all **84** expected source inputs
and `reload_required=false`. Its process start is **23:32:00 local**, because
that PID initially belonged to the held launcher; Python readiness was recorded
at 23:34:19. This earlier process start does not imply overlapping old/new agents.
Session **5318** was retained; cycle count advanced from 40126 to 40127.
The pre-signal pending choice and post-ready state are separately hash-bound in
the receipt, not claimed byte-identical across an ordinary executed cycle.

Canonical bridge transaction:
`/Users/v/other/astrid/.runtime/bridge-deployment/transactions/84e2a90ee7224096a58d60c7f98264d2/receipt.json`.

Paired receipt: `paired-observation-rollout-01.jsonl`,
SHA-256 `da43d3ac9bcb5ad07b87420e4906691c84bcd6883576d8c3c03658cfc257a860`.
Per-file source backups: `paired-observation-rollout-01.source-before`.
Bridge wrapper output: `paired-observation-rollout-01.bridge-output.txt`.
Post-verification: `paired-observation-post-rollout-01.json` and its reproducible
read-only verifier `verify_observation_rollout.py` in the artifact root.

## Verification

- Freshly rerun handoff/restart/bridge activation/drain/stage/launcher/source
  reconciliation/observation qualification tests: **132 passed**.
- Actual immutable old/new helper migration: **40 checks passed**, 20 per owner.
  Original pending inputs and prose survive; old helpers reject upgraded schemas
  without changing history. No observations or focus windows are manufactured.
- Prior exact-release tests remain applicable: **250** shared reader/writer
  tests; **2,304** bridge library tests, one ignored, plus integration and
  compile-time suites; **1,511** Minime tests, one skipped, 136 subtests;
  **234** deployment/controller/projector/evidence/Division/flywheel/domain
  checks; three release tests and two epistemic tests. Formatting and strict
  Clippy passed. Exact unsuccessful attempts remain in the implementation note.
- Pre/post coupled-stack receipts both passed:
  `env_receipt_1790058410132_134000` and
  `env_receipt_1790058883141_846000`. They verify process identities, manifests,
  ports 7878/7879, model live/readiness endpoints and telemetry freshness.
- The post-verifier independently rechecked running PIDs/starts, bridge binary,
  selected helper hash, exact startup inventory, configuration and absent holds.
  It observed **30 file samples over 60 seconds**: engine time and bridge arrival
  time advanced; fill ranged **70.996185%-73.053596%**. These are file-sampled
  freshness observations, not per-packet sensory admission or a causal effect.
- Bounded post-start log inspection found no ERROR/Traceback/panic markers in
  34 timestamped bridge lines and 16 Minime lines. It emitted no authored prose.
- Naturally queued Minime job
  `job_minime_1790058868248_self-study-continue` resumed ordinary source study.
  Its reader checkpoint migrated to schema 7. Astrid's existing reader checkpoint
  remained schema 6 until naturally used; no forced migration or new draft was
  created. The job completed at **2026-09-22T06:38:03.834015Z**, worker 54257,
  with no error. This verifies ordinary study continuity, not observation-feature
  uptake or the accuracy of an authored account. No benefit report was requested.

No engine, model, gateway, supervisor, visual or sensory process restarted.
Verified unchanged PIDs and start times: engine **41337**, gateway **41484**,
supervisor **41526**, model **43115**, visual **20885**, camera **98903**,
microphone **98910**, host-sensory **41661**, feeder **1502**. Managed environment,
rescue-profile and installed-plist hashes remained unchanged. No PI, intake,
reservoir, model or sensory-policy change was made.

The initial read-only inspection command omitted `PYTHONPATH=scripts` and failed
to import the wrapper. It sent no signals and wrote no state; the corrected
inspection passed. The deployment itself completed on its first attempt without
force, rollback or a failed activation receipt.

## Preservation and Remaining Work

All **197** historical canonical Astrid dirty paths retain their recorded status
and SHA-256; none was swept into this work. Canonical Minime now has the three
exact deployed source modifications listed above. Both feature worktrees and
all indexes remain uncommitted/unstaged. This rollout did **not** merge or push;
local main does not yet contain this feature. Keep the active immutable stage
and its source directory available until a separately qualified replacement.

The remaining Git task is one coordinated explicit-path review/commit/merge of
the owned paired feature, tests, changelogs, ledger and rollout evidence, while
preserving the older Astrid work. There is no remaining restart debt for this
release. New reader/draft schemas require forward-compatible recovery: never
restore an old checkpoint over newer authored state.

Controller remains paused at generation **457**, with no lease or projection.
V2 indexed-tail integrity is valid at sequence **1123111**, head
`8d9b06d15a16f05eae6b2bfd4c178f96f5f4216e3cfb8ceabe21d9292b4e28e6`;
all four V1 source streams remain immutable. No productive flywheel round,
read-current claim, automatic notebook selection or request for confirmation of
benefit was made. Observe public voluntary use without prompting an improvement
report; lack of use is neutral.
