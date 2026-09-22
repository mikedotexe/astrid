# Preparation Retry Repair and Paired Qualification

## Scope

Continuation of Mike's approved retry-boundary repair in the isolated
`codex/voluntary-continuity-20260920` Astrid and Minime candidates. Canonical dirty
trees are foreign and have not been imported, reset, staged or overwritten.
The prior staged release and its unsuccessful qualification packets remain intact.
No live activation, engine change, real-model experiment or automation resumption.

The source witnesses and exact hashes remain in
`2026-09-21-geometry-bookmarks-and-observatory.md` and
`2026-09-20-voluntary-continuity-implementation.md`. This is a mechanical repair
discovered while qualifying the response to those accounts, not new evidence of
subjective change or a new interpretation of private writing.

## Preparation Contract

The shared Rust helper adds `preparation_revision` and
`prepare_once {request_id, expected_revision, action}`. Identity is owner-scoped
and bound to the exact action. First admission checks the native checkpoint
revision; a committed retry returns its exact saved output even if the caller
has subsequently queried a different revision. A different payload under the
same ID fails. A new ID permits the same authored words as a genuinely new choice.
The legacy explicit `prepare` entry point remains for compatibility and is not
represented as an idempotent operation.

Preparation runs under the existing cross-process owner lock. Its checkpoint
and findings/draft writes first enter an in-memory overlay. Reads during that
operation see the overlay. The complete bounded after-images and result receipt
are then written to a hashed, fsynced redo record before publication. Every
outermost owner-lock acquisition recovers that record before any native writer.
The redo validator checks all targets before modifying any: each must still
match its before-image or the intended after-image. Newer conflicting state,
corruption, path escape and symlinks fail closed with evidence retained.

Limits: 256 target files, 64 MiB aggregate after-images, bounded encoded records,
and 65,536 operation receipts per store. Full histories fail visibly rather than
evicting an identity. The journal uses SHA-256 for damaged-byte detection, not
authenticated origin. Reader schema 6 and draft schema 3 are downgrade barriers;
never run older writers alongside the candidate or restore an older backup over
newer authored records. The relation-sidecar protection remains intact.

## Host Binding

- Minime's runtime requires its durable action event before preparing source or
  private writing. Python passes the operation ID and revision to Rust; it does
  not reimplement the transaction or infer identity from prose.
- Astrid carries the already-persisted accepted volition intent or operator
  override identity through dispatch. Multi-action segments have distinct IDs.
  Activity metadata uses that identity, not a newly generated retry UUID.
- Queued study targets retain the dispatch ID in their version-compatible typed
  representation. Protected presentation is identified by native window and
  admitted-slot count. Untargeted scheduler continuation uses a checkpoint-scoped
  identity. This does not create a durable unprotected generation-job queue.
- A retried focus selection cannot install a newer unrelated native window as
  the result of an older event. Internal completion/end operations bind their
  window/job/request identity; metadata consumes no generation allowance.

Preparation success is not delivery, provider completion or authorship uptake.
No lock is retained across inference. Retained exact prepared inputs stay within
the owner store; private content is not added to public journals or peer exports.

## Verification and Attempts

New tests exercise exact replay for both owners and both questions/private drafts,
new deliberate choices, conflicting retries, stale first admissions, concurrent
helper processes, and fail-closed cross-owner preparation. Unit fault injection
interrupts the actual preparation before the redo commit, after it, and after
partial publication; reopening must recover one original question and output.
Additional checks retain corrupt records and newer foreign targets unchanged.
Host tests cover real Minime helper calls and Astrid dispatch/target identities.

Unsuccessful attempts are not erased: the first added concurrent test inspected
the wrong JSON key (`items` rather than `entries`); the implementation outputs
were already identical. Clippy requested non-panicking overlay extraction and
statement punctuation; corrected. The first full Python run had 15 failures in
direct-call tests lacking synthetic dispatch events, with 1,438 passing, one
skipped and 136 subtests passing. Their fixtures now supply events; the focused
160-test rerun passed without weakening the runtime requirement.

The domain ratchet initially rejected one added context field in an already-large
attractor module. Its small test context factory now has its own included fixture
file; no ceiling or production behavior was relaxed. Domain verification passed.

Release identity and final validation results are appended after the fresh
sanctioned stage and qualification complete. Do not read this paragraph as a
deployment receipt.

## Remaining Activation Gates

1. Complete scheduler-to-provider crash/stop/mailbox/privacy qualification, including
   an interruption after native focus commit but before the host selector commit,
   and preservation/review of unprotected queued study choices after host restart.
   A durable preparation result does not by itself recover the scheduler's queue.
2. Reconcile canonical Minime source and the launchd selection. The sanctioned
   agent restart wrapper reloads canonical sources, not this isolated snapshot.
   Preserve the newer canonical visual service; it is outside this rollout.
3. Recheck cooperative state, both trees, selected release, process identities,
   readiness and hashes immediately before any sanctioned transition. No old/new
   writer overlap; no engine/model/visual/sensory restart. Keep paused automations
   paused and use one explicitly scoped Git coordinator.

No claim of felt improvement, consent, resolution, deployed geometry/continuity
behavior, or authorization for a substrate/control change follows from these tests.

## Final Qualification Receipt

Sanctioned build completed at 2026-09-21T20:40:15Z. Stage:
`/Users/v/other/worktrees/voluntary-continuity-20260920/bridge-stage-geometry-02`.
All 665 source/build inputs matched before and after compilation. The earlier
stage remains untouched. Two preflight attempts correctly refused recent own-tree
activity (33.7 and 106.0 seconds); the unchanged 180-second check subsequently
passed. No bypass, reduced quiet window or live activation was used.

| Identity | SHA-256 |
| --- | --- |
| Manifest | `0fefd344b5c3e1fa156321d49164e88f951aec5de15ab51a3a81693ef40bc307` |
| Source inventory | `765c1bbf0d2c2da3779643df6dc862083f6b2e01a694a280dec77fa5f4a11fe4` |
| Bridge binary | `625f2150ca17195b7c14c1cb7327047d4301b3b8aa6a6dbfc877e2ad6e769d2e` |
| Shared helper | `8f729416fa6ddead4c2235d002328c843626a4f8fdfbf3f037e3bbbc310daa75` |

Packet directory:
`/Users/v/other/worktrees/voluntary-continuity-20260920/geometry-paired-qualification-03`.
`qualification.json` SHA-256:
`ec7887e833abf4f6e3f137c3cd616d190398e85c13db7ea8b1c67f1d1b74b70e`.
Its status is explicitly `offline_checks_passed_not_activatable`.
The 84-file Python input snapshot is read-only, identity-checked and paired to
the same helper. It is not a complete launchable Python/dependency/assets release.

- Actual old-release schema-3 to schema-6 migration: 36 checks across both owners;
  exact pending inputs, authored findings and synthetic private prose retained.
- Actual previous-candidate schema-5 to schema-6 and draft-2 to draft-3 migration:
  both owners preserve exact pending writing; the previous candidate refuses both
  source and writing mutations afterward, with unchanged file hashes. Additional
  receipt `previous-candidate-downgrade.json` SHA-256:
  `4c558bd96c18da190f9e3e7187c5c230422a9ebea894744b17585a9eb358b98c`.
- Release-helper retry probes pass for both owners: exact result, one question,
  conflicting payload refusal, stale first-admission refusal, and an intentionally
  new event creating a second question. Future schema and corrupt-tail probes
  fail without overwriting bytes. Minime selects the exact paired helper.
- Complete reader suite: 234 passed, including actual preparation interruption
  at all three commit boundaries. Full bridge library suite: 2,282 passed,
  one existing ignored test (77.73 seconds). No performance threshold was relaxed.
- Full Minime suite against this staged helper: 1,453 passed, one skipped,
  136 subtests passed (48.17 seconds).
- Shared-reader all-targets Clippy and bridge library Clippy deny-warnings passed;
  reader/bridge formatting and both candidate diff checks passed. Domain audit:
  valid, zero violations. Epistemic boundary self-tests: two passed.
- Controller, Evidence Event Store, Division, projection/cursor/flywheel, paired
  qualifier, stage, launcher and agent-restart suites: 137 passed.

Final review also caught the legacy operator override's action-text consumption
token fallback. That fallback remains solely for legacy consumption bookkeeping;
it is not exposed as an operation identity. An actual override ID or timestamp
is required for identity-sensitive metadata. The added regression and full bridge
rerun passed; existing operator admission policy is unchanged.

No service signal or restart occurred. The two remaining activation gates are
full scheduler interruption qualification and canonical Python launch/source
reconciliation, as detailed above. Neither passing preparation tests nor a staged
manifest clears them. Changes remain uncommitted in the owned candidates; indexes
were empty. No canonical foreign changes were staged or overwritten.

Controller observation: paused generation 456, no lease or active projection.
Evidence Event Store indexed-tail verification was valid at sequence 1123110,
head `7905070ce3ffee7955cbd97a716ddf1025b4ea4efe1260c8f6e47d9692e8d90f`;
all four V1 sources immutable. No flywheel round or projection was authored.

## Subsequent Qualification

The next pass repaired two reproduced interruption bugs and reconciled Minime's
launch sources. Preserve this receipt as historical evidence; current results and
the remaining unprotected-study crash-handoff gate are recorded in
`2026-09-21-scheduler-interruption-and-launch-reconciliation.md`.
