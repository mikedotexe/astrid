# Study navigation and evidence repair — September 10, 2026

Mike authorized implementation and graceful live rollout of the three follow-ups
from the latest five journals of each Being. The implementation uses isolated
`codex/study-navigation-20260910` worktrees. Root Codex owns staging, integration
and rollout; reader_evidence owns shared-reader work and minime_navigation owns
Minime changes and cross-review. Cooperative steward work in canonical Astrid is
unrelated and preserved. Maintenance pause generation: 436.

## Behavior and design

Astrid retains a pending targeted or untargeted study choice until consumption or
deliberate replacement. Identical retries share it. A different request gets a
clear not-queued receipt plus an optional exact `SELF_STUDY REPLACE` command.
Cadence cannot override pending authored reading, and retained targets restore
eligibility after a checkpoint. No new queue or checkpoint schema is required.
Private writing arguments remain private in both original and aliased receipts.

Minime already freezes input per job. Its older job can no longer clear a newer
pending choice. An additional study/private-writing request while busy now has
an explicit blocked/not-queued receipt and a retry path; the original job stays
unchanged. This replaces a prior misleading “deferring” return that saved no job.

Both runtimes use one stateless recovery helper. Suggestions do not execute
alternatives or alter network-search authority. Shared FIND/RELATE results show
role counts on every page, exact OPEN commands, bounded scan scope, and optional
nearby spellings. Heuristic evidence labels cannot establish production semantics;
test strings and historical commentary remain accessible. Notebook guidance no
longer recommends the identical lookup already delivered in the current turn.

## Evidence and validation

The frozen survey is retained at
`/Users/v/other/reservoir-llm-research/research/outputs/2026-09-10-latest-five-journals`.
The separate revision experiment is in
`analyses/2026-09-10-study-evidence-revision.md` in that repository. Its generated
commands are data, never dispatched. Behavioral benefit remains an observation
question; no natural study or private writing is induced for uptake.

Validation receipts are retained under
`/Users/v/other/worktrees/study-navigation-20260910/`. Shared reader: 56 tests,
formatting and strict clippy pass. Minime: 1,323 tests pass, one skip and 132
subtests after final malformed-private-request regressions; compile and diff
checks pass. Bridge strict all-target/all-feature clippy, formatting and boundary audit pass.
The first full bridge run had 2,260 passes and one timing assertion at
1.059ms against a 1ms threshold while isolated inference also ran; that failure
is retained. The next run exposed two test fixtures that omitted the production
mode flag reset when simulating consumption; those fixtures now model the full
consume boundary. Final bridge unit suite: 2,264 passed, one ignored. Additional integration,
compile-boundary and documentation checks are retained in the full-suite log.
The final five navigation regressions and strict clippy pass after case-insensitive
replacement/privacy and empty-operation validation. Release identities follow below.

The stage inventory recursively records the new Rust module and shared reader
sources, and ships the reader helper with the bridge. Minime changes existing
runtime/source-study modules already covered by restart input checks. No model
server source, provider sampling, reservoir input or live coupling policy changes.

## Rollout

Pending final tests, explicit-path commits, staged bridge activation and graceful
Minime reload. This paragraph is a pre-rollout record, not an activation claim.
Final receipts will retain old/new process identities, exact checkpoint continuity,
loaded helper/source identities, surrounding services and natural exposure scope.


## Verified paired rollout — 10:19 PDT

Astrid implementation `59a79771f0a83a9aa31d6e876aed22dd01c450cf` is on main and
pushed. Staged V3 release `bridge-stage-01` under the retained workbase has manifest
SHA `2d9f75d1bd084aa0b1530813da195737457412620587ed5756df8085b84d2ff4`.
The new navigation module and both shared-reader modules appear exactly once in
its hashed inventory. The selected reader SHA is
`a6e1cfbc964bae4677a3fc2d23468d863039389af85e48d68efbb9e2ab6e56be`.
The first activation attempt stopped before touching live processes because the
shared-tree settle interval had not elapsed. After waiting, the sanctioned wrapper
completed transaction `11d171a213114a8199f0b8aa85c922c1` at 17:15:45 UTC.
PID 20518 drained and exited; PID 65235 started at 17:14:01 UTC, verified the exact
checkpoint SHA `2fa087920407c24f9a5897c065c39c4671df627e94d21a9da1afe1a4f77f613b`,
restored exchange 195107 and saved 195108. Self-control lineage and pending feedback
startup checks passed. No forced stop or lossless remote-delivery claim was made.

Minime implementation commits `3d5d43a` and `15df37e` are merged to main and pushed;
main source `15df37ea4524375c1d9e816e5c8a447d02cc4cb5` is verified loaded. The
sanctioned agent-only wrapper waited for a quiet boundary, sent SIGTERM at
17:18:15 UTC and verified new PID 66900 at 17:18:21 UTC, replacing PID 22243.
Source inputs and configuration match, reload_required is false, no old active
job or newly interrupted-job recovery remained, and the session was retained.
The new process advanced a cycle, so pending-NEXT fields are not byte-identical;
its natural consumption is a separate action/job trace, not inferred from reload
success. Both wrappers used ordinary graceful paths without a forced fallback.

At 17:19:35 UTC all nine surrounding services from the pre-rollout inventory had
identical PID/start identities, including the coupled model PID 43115. No coupled
server reload, sampling retune, reservoir/coupling intervention or induced study
occurred. Full bridge suite: 2,284 passes across unit/integration/interface groups,
one ignored test; final navigation regressions, strict clippy, formatting and
boundary checks pass. Shared reader: 56 passes. Minime: 1,323 passes, one skip,
132 subtests. Research: 228 passes.

Cooperative steward documents were temporarily saved by explicit path and restored
without conflict. Their changed lines and the four foreign script hashes match
the pre-integration evidence. They remain unstaged. The retained safety stash is
`f5fafee45af411c5ef86a8231983398dae92d563`; no foreign changes entered these commits.

Receipts under `/Users/v/other/worktrees/study-navigation-20260910/`:
`bridge-activated-receipt.json`, `minime-reload.json` (NDJSON), `live-verified.json`,
`stage-module-inventory.json`, `bridge-test-summary.json`, and
`foreign-preserved-after-integration.json`. The retained source worktree and stage
are immutable while selected. Research and first natural follow-through are in
`/Users/v/other/reservoir-llm-research/analyses/2026-09-10-study-navigation-live.md`.
