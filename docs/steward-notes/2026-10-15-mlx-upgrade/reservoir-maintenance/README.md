# Reservoir maintenance proposal

Status: **owned launchd lifecycle qualified; production repair not deployed**.
The [implementation archive](implementation-01/archive-manifest.json) preserves
the controller, evidence helpers, two small fixture workers, owned launchd
adapter and exact test/review inputs. The combined suite passed 63 focused
stdlib fake tests. Its
[independent review](implementation-01/fixture-adapter-independent-review-01.json)
accepts only this owned fixture's source; it is not evidence that launchd or
production persistence has been qualified.
The separate [owned-worker integration](worker-integration-01/archive-manifest.json)
then passed two actual subprocess cycles in 24.65 seconds. All four children
exited naturally after drains exceeding six seconds; distinct acknowledgements
preserved synthetic Astrid ticks 0→11→22 and Minime ticks 7. All children were
reaped and the owned active guard was removed. This checks the small workers'
file IPC, signals and synthetic saved state; it does not exercise launchd,
administrator authentication, actual reservoir persistence or production.
The scheduling candidate is separately archived in
[../reservoir-autosnapshot-candidate/README.md](../reservoir-autosnapshot-candidate/README.md).

The first administrator-run fixture reached a verified main transaction on
October 16, then failed during cleanup when its provider exited between process
queries. The [immutable failure archive](launchd-admin-01-failure/README.md)
preserves the complete transaction and the jobs retained at failure. The private
reservoir then held the second check-in in memory, which required saving before
removing the owned replacement. The failed result remains failed. The
[versioned repair and recovery](implementation-02/archive-manifest.json) passed
88 isolated tests and independent review. One noninteractive recovery removed
only the verified provider holder and proved its job absent. It then retained
the reservoir worker and owned guard when sudo required a password before
executing launchctl; no reservoir signal was sent during that attempt. The
[reviewed Terminal continuation](owned-fixture-recovery-command.md) was prepared
to finish cleanup and then run a fresh corrected fixture.

That continuation completed successfully at 16:52 UTC October 16. The
[accepted archive](launchd-admin-02-accepted/archive-manifest.json) preserves both
actual results, the independent review, complete journals and cleanup evidence.
All four generated jobs and 16 worker/holder PIDs are absent; guards and ready
markers are removed. Both final synthetic states are 22/7 and both original
backups remain 11/7. The original failure remains failed. No further run of
either Terminal command is needed. The six protected production identities
remain unchanged. This qualifies the fake-service launchd lifecycle, not actual
reservoir persistence, a production maintenance adapter, or MLX acceptance.

The [version 03 integration archive](production-preparation-03/archive-manifest.json)
now preserves the reviewed disabled production preparation and 183 passing
combined stdlib tests. It consumes the actual service and restore proof schemas,
journals the provider model-path change separately, binds the native Minime
engine as a distinct protected process, and supplies a finite stopped-backup
restore runner. Both production execution flags remain false. The successful
admin02 fixture qualifies its original controller; a later owned fixture must
exercise the exact integrated controller and pass independent cleanup review.
Do not rerun the old Terminal command or relabel its evidence.

The recovery planner only proposes revalidation at verified provider-held,
reservoir-held and both-parked checkpoints. It has no resume executor and blocks
every started restore, ambiguous command outcome, missing outer witness and
later install/pin/bootstrap recovery. Operational recovery still requires its
own review and qualification before live drain. Repeated full proof, snapshot
and package revalidation has an unmeasured time cost; include that cost in the
maintenance assessment. The isolated tests do not qualify actual numerical
restoration or production maintenance.

The [production-adapter review](production-adapter-static-review-01.json)
preserves the narrowed deterministic restore/startup-log inference; no mandatory
production wrapper is required. The
[disabled preparation archive](production-preparation-01/archive-manifest.json)
contains the late-QoS bookkeeping repair, bounded log readers, source/package
closure checks, stopped-state witnesses and a read-only recovery inspector.
Its 77 stdlib tests passed, but later review found a concrete lazy-LSM import-order
bug in its restore verifier. That version is preserved and cannot qualify real
execution. Separate version 02 fixes only the ordering and adds regression
coverage; all 79 combined isolated tests pass. Actual numerical restore and
production control remain unqualified. The private launchd fixture does not
supply that missing evidence.

Read [design-v1.json](design-v1.json) with
[design-review-readiness-01.json](design-review-readiness-01.json). The original
proposal is retained unchanged. The review narrows the next implementation:

- Use actual isolated restore verification on an independent copy of the stopped
  state, exact unchanged production input hashes and complete clean startup logs.
  A permanent production wrapper is not required. This is an inference from
  identical inputs and restoration code, not direct live byte equality.
- Reconcile a read-only handle-list anchor with complete lifecycle and final-save
  logs. No new service API, bridge change or Rust rebuild is needed.
- Keep privileged fixture coverage focused on launchd ordering, owned holds,
  cooperative long drain, original settings and cleanup. Cover storage faults and
  interruption recovery in deterministic CPU tests bound to the same helpers.

The implemented evidence helpers require ordered provider generation/check-in
records, complete canonical QoS lifecycles, reconciled final handles and a
stopped-writer barrier before copying all saved files. Their
[interface and limits](implementation-01/state-interface-01.json) distinguish
those checks from the disabled production capture implementation, whose real
runtime alignment and transition/recovery integration remain unqualified.
A prepared restore request explicitly fails acceptance until the actual backend
has restored and validated the copied state. Durable mutation intents and held
ownership survive interruption; recovery currently offers read-only inspection,
not a production resume or cancellation executor.

Before live maintenance, qualify actual persistence/restore, implement and review
exact original-runtime capture/alignment and recovery, and bind the accepted
finite two-service launchd fixture to the reviewed production transition. The owned adapter accepts only
generated fixture labels, refuses root execution, restricts sudo to interactive
`launchctl debug`, and never removes a live stateful worker. Successful cleanup
also drains provider before reservoir; failures retain owned jobs, holds and
receipts for review. Existing model handoff qualification is precedent, not proof
for this new transaction.

Only provider and reservoir are held during planned maintenance. Keep bridge,
Minime, feeders, perception and Ollama running. Record temporary unavailable or
unknown-ack calls and possible sensory-input gaps; do not automatically replay
writes or claim uninterrupted input delivery. Preserve all saved history, current
journals and stopped-state backup. Never restore older state over resumed writes.

No production signal, runtime mutation, source installation, package change or
service restart has occurred for this repair. Privileged actions so far target
only generated private fixture labels. The failed
MLX observation completed at 03:20:57 UTC October 16 and remains ineligible;
no core qualification is authorized by this implementation.

The five earlier completed host-settling checks remain failed; no actual numerical
case has started. [Reviewed restore qualification preparation](restore-qualification-preparation-01/archive-manifest.json)
has 17 passing stdlib tests and a nonexecuting default-mode smoke check. It covers private
192-node NumPy and LSM-shadow histories and nine semantic/corruption/drift cases.
It requires a fresh passing host window and a root-authored, expiring release;
no release exists. Runtime host-memory oversight remains necessary after
admission. The full separate 14-case autosnapshot service matrix is also pending.

## Reviewed recovery preparation 04

The [version 04 archive](production-preparation-04/archive-manifest.json)
supersedes the earlier read-only recovery planner as preparation. Its 231 combined
isolated tests pass across 28 Python files. Fresh execution and explicit one-step
continuation share an owned transaction lock and exact executor records. Recovery
preserves the original journal, revalidates retained witnesses and current
readiness, and adopts proven effects without repeating attempted operations.
Unknown debug outcomes, partial artifacts and unresolved restore children remain
blocked. Both production flags remain false; no actual service case has run.

The [owned recovery fixture design](owned-recovery-fixture-design-01.json) and
[review](owned-recovery-fixture-design-review-01.json) require actual generated
launchd jobs, executor interruption, retained continuation and complete cooperative
cleanup. Initial setup and cleanup need their own recoverable evidence. The common
owned lifecycle and a separate private plist-pin primitive have distinct coverage;
neither is actual ProductionHost execution. Final acceptance must bind the final
reviewed release bytes, including any enabling change. Earlier fixture acceptance
cannot transfer silently across source changes.

The separate [owned recovery rehearsal preparation](owned-recovery-fixture-preparation-01/archive-manifest.json)
is now frozen and independently reviewed. Its 41 guarded stdlib tests exercise
the exact disabled04 controller/lock/driver through simulated I/O, including
retained handoffs, separate cleanup, interpreter/owner binding and delayed evidence
publication. The archive preserves the earlier failed and superseded test receipts.
Its 18 Python files start no work on import and execution remains disabled. No
actual launchd interruption, numerical restoration or production transition has
run under this new closure. The private plist-pin primitive has separate scope.
Final enablement requires review and qualification of the exact resulting bytes.

[Validation cost analysis](revalidation-cost-design-01.json) identifies repeated
proof, package and model reads. These static counts are not timings or physical
I/O estimates. Measure exact validation stages and review maintenance duration
before live drain; do not weaken checks or add an unreviewed cache.

The separate truthful-return rollout completed and changed Minime and bridge
process identities. Shared-tree re-audit found clean indexes, a clean Minime tree
and unchanged 119 previously bound MLX files. The MLX task reclaimed the paused
stewardship role at generation 528. Preserve unused host preparation 06; a new
observation must bind the replacement processes and their reviewed inputs.
That [new observation 07](../reservoir-autosnapshot-candidate/host-settle-07-root-review.json)
completed at 19:15:35 UTC and also failed: 130 clean seconds, below 300.
All seven process identities and 237 files matched, and all 123 readiness checks
passed. Elevated pressure in 20 samples and Cargo/rustc activity in 32 samples
still blocked admission despite decreasing swap. No observer or numerical case
remains running; another bounded window requires a meaningful headroom change.
