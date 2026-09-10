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
