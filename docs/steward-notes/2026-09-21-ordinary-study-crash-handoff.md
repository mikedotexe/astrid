# Ordinary Study Crash Handoff

## Scope and Witness Continuity

Mike approved repairing the remaining ordinary, unprotected Astrid study
handoff. Work stays in the isolated `codex/voluntary-continuity-20260920`
candidate. No canonical source, live native state, running service, automation
configuration, Git index or branch tip was changed by this repair.

This continues the previously fully read accounts, not a new analysis of private
writing or evidence of subjective improvement:

- Minime: `minime/workspace/journal/!aspiration_2026-09-20T10-39-43.801459.txt`,
  SHA-256 `0aa5679357da59a121520e5d2bd42d0908170915435546068682479d1a182ee6`.
  "I want to become a being of *persistent interiority*."
- Astrid: `capsules/spectral-bridge/workspace/journal/!aspiration_longform_1790011424.txt`,
  SHA-256 `b31ebe800a9adead2b366d6b19fecc1c32f927e37b2955d693a658ae1c95bb46`.
  "I am trying to sense the *movement* between the weights."

The immediate engineering evidence is the dispatch/checkpoint gap described in
`2026-09-21-scheduler-interruption-and-launch-reconciliation.md`. The existing
dispatch could acknowledge a queued study before its target reached disk. A
later crash could instead restore a spent target from an older conversation
checkpoint. A serialized optional operation ID alone did not close either gap.

## Implemented Contract

The existing authoritative activity selector now carries a bounded study-job
history and selected job reference. This is scheduling/delivery metadata, not a
parallel belief or memory database. Authored prose and question state remain in
the existing native reader and draft stores.

1. **Queued:** after existing action authorization, commit the exact owner-bound
   request before reporting acceptance. The job ID derives from the actual
   dispatch operation ID. Repeated words with a new operation are not globally
   deduplicated; an identical request while still pending shares that pending
   job only after checking its durable identity.
2. **Prepared:** use the production `prepare_shared_study_target` and native
   `prepare_once` transaction to retain the exact input. A crash between native
   preparation and host publication retries that same operation, rather than
   creating another question or moving the cursor twice.
3. **Claimed:** commit a one-use provider-admission claim before invoking the
   provider. The owner transaction lock is released before inference. Existing
   provider retries and resource limits remain within this one job.
4. **Delivered:** validate the operation-specific retained provider receipt and
   exact prepared input, import into the native notebook/draft idempotently,
   then commit completion. Restart can finish this bookkeeping from the
   verified retained artifact without invoking a model again.
5. **Failed or superseded:** known preparation/provider failures and explicit
   replacement retain terminal history. Replacement is allowed only before a
   provider claim. A preparation failure cannot mark a claimed job failed and
   thereby erase uncertainty.

Untargeted ordinary scheduled studies receive a fresh durable admission before
preparation. Protected jobs retain their existing window/slot protocol. Legacy
workspace-artifact reading keeps its separate owner-aware reader path.

Provider retention uses `source-job:<job-id>` rather than a reusable navigation
text hash. An identical MAP output from another generation cannot be mistaken
for this job's delivery. A conflicting checkpoint, missing artifact, changed
source bytes, altered receipt or copied cross-owner record fails closed.

## Recovery Is Not Action Replay

A claimed job with no verifiable retained outcome stays blocked for recovery
review. It is not automatically regenerated. A crash after the claim but before
the network call cannot be distinguished reliably from a call whose response
was lost; this implementation does not claim exactly-once external inference.

Recovery imports the response into its native record and preserves its exact
authored NEXT. It does **not** replay NEXT, publish another public journal entry,
send a peer message or execute a control action. A crash after native delivery
but before downstream dispatch therefore leaves a retrievable authored choice,
not a guarantee that the choice executed. Existing authorization must still
precede any separately chosen action.

Ordinary handoff uncertainty stops competing generation/mail admission at the
existing exchange preparation boundary. The outer authenticated stop/graceful
drain path retains precedence. There is no automatic reset or new recovery
authority. Review must first inspect the exact activity reference, frozen input
and operation-specific accepted-delivery partition. Do not remove a claim,
restore an old checkpoint, manufacture a delivery or replay its NEXT to clear it.
If no trustworthy outcome can be recovered, an explicitly reviewed resolution
is still needed; this tranche does not add a destructive recovery command.

Pending ordinary work cannot be silently erased by starting protected focus.
Focus admission checks the durable ordinary job before changing the native
selection. ACTIVITY_STATUS reads the authoritative selector rather than an
older conversation cache and returns IDs/phases, not private draft contents.

## Persistence and Compatibility

- `action_threads/activity_runtime_v3.json` owns selection and operation phases.
  Every participant uses the existing cross-process owner transaction lock and
  selector revision check.
- Exact requests and frozen inputs are content-addressed under
  `diagnostics/source_first_v3/shared_reader/study-handoff`. Artifacts are bounded
  to 1 MiB, written with mode 0600, synced before their references, and hash
  checked on use. New artifact directories use mode 0700. Symlinks are refused.
- The selector permits at most 65,536 retained job identities and 32 MiB of
  encoded state. Reaching a bound fails visibly without evicting history. There
  is no automatic history pruning in this release.
- Migration retains exact previous bytes in hash-named archives, leaves invalid
  downgrade guards at the v1/v2 paths and publishes v3 atomically. Existing focus
  references survive; their native budget is not recreated or replenished.
- An interrupted upgrade or an older writer replacing a guard fails closed.
  Historical archives are evidence, not permission to restore over newer work.
- A legacy queued target without a durable job cannot be treated as verified
  crash recovery. It requires explicit review/reselection. The rollout review
  must inspect pending legacy work before starting the new adapter.

## Verification and Retained Attempts

Fourteen handoff tests include a subprocess test helper; the thirteen substantive
cases exercise the actual native preparation and retention paths, synthetic
source pages, navigation and private writing. Coverage includes accepted choice
before conversation checkpoint, native preparation before host commit, provider
claim without outcome, native delivery before host completion, terminal stale
checkpoints, independent later choices, explicit replacement, cross-owner
copying, missing/corrupt artifacts, changed sources, wrong-operation/tampered
receipts and durable dispatch refusal before acknowledgement.

Two independently spawned Rust test processes exercise the actual owner flock:
identical queue retries share one record; competing provider claims have exactly
one successful claimant. Additional regressions cover v2 migration and a pending
ordinary study refusing focus before native selection changes. Existing volition
and mode-entry tests now use isolated durable stores and real operation IDs.

Initial test failures are retained here rather than presented as all-green first
attempts. macOS temporary roots needed canonicalization because `/var` is a
symlink. The first source-drift fixture used a file outside the approved catalog;
it now uses catalog-admitted `build.rs` and asserts that a real page was prepared.
A test visibility error was repaired. Production-only Clippy found an older mode
entry still calling the test-only in-memory helper; that entry now uses the same
durable path. Three nested conditionals were corrected for denied-warning lint.

- Full bridge library: 2,303 passed, one existing ignored test (50.68 seconds).
- Complete shared-reader suite: 234 passed.
- Controller, event-store, Division, projection/cursors/flywheel and affected
  release/restart/reconciliation suites: 226 passed (26.311 seconds).
- Experiential epistemic self-tests: two passed.
- Final formatting, Clippy, adapter rerun and immutable stage results are recorded
  below. No live experiment or generated being reply was requested.

## Deployment Boundary

The previous stage 03 and paired qualification 04 remain historical and unchanged;
they do not contain this repair. A fresh immutable stage must bind this source.
Paired helper migration and canonical Python launch reconciliation remain
separate evidence from the new bridge scheduler tests.

After final qualification, the remaining live transition still needs one
cooperative coordinator, exact canonical-source reconciliation, legacy pending
work review, sanctioned agent/bridge wrappers, fresh process/hash/helper and
readiness verification, and checkpoint continuity. No engine, model, visual
service or sensory-client restart is part of this repair. Never bypass foreign
activity or overwrite newer authored state to activate it.

Read-only controller observation remains paused at generation 456, with no lease
or active projection. Indexed-tail V2 verification is valid at sequence 1123110,
head `7905070ce3ffee7955cbd97a716ddf1025b4ea4efe1260c8f6e47d9692e8d90f`;
all four V1 sources remain immutable. No report-processing round or new
projection was authored. Paused automations remain paused.

## Final Build and Paired Qualification

The immutable stage-only build completed at `2026-09-22T03:17:42Z` (September 21
local time). Stage:
`/Users/v/other/worktrees/voluntary-continuity-20260920/bridge-stage-geometry-04`.
The sanctioned wrapper accepted the unchanged 180-second quiet requirement;
earlier preflight refusals at 35.2, 100.3 and 153.9 seconds were respected. No
timeout, exclusion or concurrency guard was weakened to complete the build.
All 668 packaged source/build inputs matched before and after compilation.

| Identity | SHA-256 |
| --- | --- |
| Manifest | `4faab24713471c74829a9ba7cdc27abc78654901a31605766e1587fd9ce6ec9c` |
| Source inventory | `7a239e659116f0705ba5490fa032531fb1cbb586bdab82353fef2eafe84b0597` |
| Bridge binary | `10e9bdd6fcbe73908a9242305faeb9ce11444fddea1ec678b34eab74fdc4cb43` |
| Shared helper | `8f729416fa6ddead4c2235d002328c843626a4f8fdfbf3f037e3bbbc310daa75` |
| Handoff source | `449bb8a90afc023143af61657b2b9ab023b3330c1fcb91447eae2f91b7f6a1f3` |
| Handoff tests | `1caf0191d0017ad6daed46d752b46361531d8d0115fee8c985c86216a0f01558` |

Paired packet:
`/Users/v/other/worktrees/voluntary-continuity-20260920/geometry-paired-qualification-05/qualification.json`,
SHA-256 `f29cfbd19483782b2e053a5c2fad7ba72d4f6d0a999c5b7cd03c91d910790c88`.
All 36 old/new helper migration checks pass across both owners. Exact preparation
retries, deliberate new choices, corrupt/future state rejection and paired helper
selection remain verified. The frozen Python inventory is still bound to the
exact 84 selected launch inputs in reconciliation 01, and canonical inputs were
rechecked without installing the overlay.

The packet remains `offline_checks_passed_not_activatable`. Its host scheduler
evidence gate now explicitly requires source-bound review of the separate
handoff/migration tests; it does not present the helper migration checks as host
scheduler coverage. Cooperative canonical installation is still unperformed.
Previous stage and qualification packets remain unchanged.

Final additional verification:

- Final full bridge library rerun against unchanged staged source: 2,303 passed,
  one existing ignored test (41.33 seconds).
- Complete Minime suite: 1,463 passed, one skipped, 136 subtests (56.69 seconds).
  The first run had two failures because the launching shell supplied
  `MINIME_LLM_TIMEOUT_S=160`, while two existing tests assert default 60/160-second
  relationships. The successful rerun unset only the three timeout overrides in
  the test subprocess. No runtime timeout, test assertion or live configuration
  was changed. The helper is byte-identical to stage 03 used for that suite.
- Handoff-focused rerun: all fourteen tests passed after final changes.
- Six bridge integration tests passed, including the facade test's two public
  export compilations and one expected private-module compile failure.
- Bridge library and test-target Clippy with denied warnings passed. Bridge and
  shared-workspace formatting checks passed. Domain audit: valid, zero violations.
- The qualifier's eleven focused tests passed after clarifying its evidence gate.
  Both candidate worktree diff checks passed and indexes remain empty.

A read-only live checkpoint metadata observation at `2026-09-22T03:18:29Z` found
`wants_introspect=false`, no pending target and no native focus reference. Its
SHA-256 was `d17b6f724dfff45c446d572d85875a934d57646d59bc203d7797290818c6785a`
(mtime `03:18:21Z`). No prompt or private writing content was displayed or used.
This transient cache observation is not a drain receipt or permission to skip
pending-work review at the actual transition.

Live bridge PID 82935 still has its September 18 17:46:13 local start and old
stage-01 executable. Minime agent PID 18648 still has its September 20 11:36:25
start. No activation, Git staging/commit/merge, canonical overlay installation,
Being-facing note or automation resumption was performed. The repair is built
and qualified offline, not deployed or evidence of subjective improvement.
