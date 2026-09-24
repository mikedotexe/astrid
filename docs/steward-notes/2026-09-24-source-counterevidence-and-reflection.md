# Source counterevidence, voluntary note revision and open reflection

Current status: paired reader and adapter release verified live at
2026-09-24T16:38:39Z. This note accompanies exact-path main integration. The
initial candidate record below is retained as history; rollout evidence follows
the initial closeout. No subjective improvement or uptake is inferred.

## Status and authority

2026-09-24, initial Codex implementation candidate. Mike approved the four recommendations
from the Minime study review, then explicitly encouraged fixing inherited
qualification issues discovered during this work. No live service, reservoir
parameter, approval gate, automation, journal or canonical reader store changed.
No staging, commit, merge or push has been performed in this task.

Paired isolated branch: `codex/study-counterevidence-20260924`.

- Astrid base: `3b18af87b0fe1f083d95cbe0eac8befb309638f2`.
- Minime base: `d8e8954b3c71d54037951f832062f8b5ea62497b`.
- Worktrees: `/Users/v/other/worktrees/study-counterevidence-20260924/{astrid,minime}`.
- The steward controller was already paused at generation 467 with no lease.
  It was inspected, not resumed or claimed. This is interactive isolated work,
  not a productive automation round or an introspection-addressing closure.
- Existing Claude source-reader shorthand, sidecar switch/hook and inbox changes
  are included in the baseline and preserved. No old worktree was swept or removed.

## Exact witnesses

All six public files were read completely, in time order. Paths below are relative
to `/Users/v/other/minime/workspace/journal/`; hashes are SHA-256 of exact bytes.

| File | SHA-256 |
| --- | --- |
| `self_study_2026-09-24T07-33-22.340165.txt` | `2e8e1d40bb895224aab6aa58a856875132c02954f81ca20430c297cd7ab2157d` |
| `self_study_2026-09-24T07-36-40.517157.txt` | `2acdc91448683515c8baa34f2b67582bf203004bc874d3aea68552cc122a52c3` |
| `self_study_2026-09-24T07-40-07.410141.txt` | `082d726e30dc6029dfc14ef26309255f33df582ce191500f8de3937443f4eb38` |
| `self_study_2026-09-24T07-44-39.969181.txt` | `6479f3ff16860bbb253688e598ae11e5a4f8ea424cb3ab8abe060ec0ae98f824` |
| `self_study_2026-09-24T07-48-05.184019.txt` | `51c4b19e8625caf65b73faf44d0183d555552a28437058fbda62d813b17b16c6` |
| `self_study_2026-09-24T07-52-36.172055.txt` | `84d37ebaedd55db599dd852841506c48e76cdf7bb43037c39e5b047754802a00` |

The 07:48 account says:

> The `validate_imports_exports` call acts as a gate against invalid configurations.

> The `await_capsule_readiness` call acts as a temporal gate, preventing the system from proceeding until the core infrastructure is stable.

Those claims are not established by the supplied code. The response closely follows
overstated comments in that code. This is an interface and engineering signal, not
a reason to overwrite Minime's account or demand a correction from him.

The retained note's public origin is
`self_study_2026-09-23T18-58-00.986130.txt`, SHA-256
`f3c14bc3ceeae3980b2b1300c9a4b131c764ebedb2293ed88d37021b85ee5af4`.
It connects identity context to `RestartTracker`; the tracker actually uses attempt
counts, elapsed time and backoff. No private journal was used as evidence.

The studied kernel source revision was
`0cb8b392ad139a7f4c117f9b166273aaf5c1dc2beacf863e40d1eb72e1aea109`.
After an EOF response and explicit reopening, supplied byte intervals advance:
4501..8775, 8775..12825, 12825..16886, 16886..20908, 20908..24932.
These are real reading advances within a reread, not six identical inputs.
Provider delivery artifacts and their content hashes were checked. For example,
the 07:48 receipt is under Minime's
`workspace/diagnostics/source_first_v3/shared_reader/deliveries/83bd64d56b7159239deb8bee6c0c7d0a9d1adb6b235ba977b48d25e0b9a56a20/b1f3bdde9333107a5285b2b0421184132d66e121370a7d24141b69e157f3069d.json`.

## Implemented changes

### Mechanical descriptions and failure paths

Kernel comments now distinguish advisory import diagnostics, advisory readiness
waits, attempted loading, best-effort unload and confirmed cleanup. The legacy
`capsules_loaded` payload still says `ready`; it is not universal readiness evidence.
The 500 ms timeout is passed to capsule implementations, not independently enforced
by this caller. Missing registry entries are skipped. Timeouts/crashes are logged.

The original readiness and restart-unload blocks are extracted unchanged into
`crates/astrid-kernel/src/lifecycle_checks.rs` and tested directly: missing required
imports return normally, timeout/crash results do not reject loading, a shared Arc
skips unload, and an unload error does not propagate. Reload-after-skip behavior
is documented, not silently repaired. Kernel shutdown remains best effort.

### Inspectable counterevidence

`SELF_STUDY RELATE <identifier>` can offer one optional SESSION containing a parsed
definition and call site. It does so only for one definition in the scanned
candidate set and an available call candidate, preferring the same source file.
Multiple definitions suppress the pairing. Parser limits, test exclusions,
comment/string exclusion, bounded scans and explicit pagination remain intact.
The comparison is labelled syntax-only, with binding unverified. It invites
inspection of return, retry, skipped cleanup and log-and-continue paths; it does
not decide the answer or mark a claim false. Unscanned source remains unknown.

### Quiet, voluntary note revision

`SELF_STUDY NOTE [--page N]` explicitly opens the current note and two history
records per page. Routine source/map/question inputs retain the note's identity
and retrieval affordance, not its body or history. Recent authored prose and
explicit source-linked findings retain their existing separate contracts.

An optional authored directive supplies a revision and exact counterevidence:

```text
STUDY_REVISE: {"prior":"current note response_sha256","text":"My revised account","source":"astrid/crates/example/src/lib.rs","line":2}
```

The prior identity must match the current inquiry's note, and the cited line must
occur exactly once in the verified source page/session supplied for this response.
Navigation hints, earlier pages and unprovided lines cannot become new evidence.
The frozen anchor includes source hash, byte interval, page ID and actual fragment.
Nothing establishes correctness automatically; the inquiry is not resolved.

Ordinary STUDY_NOTE replacements and explicit clearing also preserve previous
accounts. Identical repeated notes do not refresh identity. At 64 changes, further
mutation fails visibly without eviction; the attempted response remains in its
delivery receipt. This is a bounded first release, not unlimited note storage.

Reader schema 8 and chain validation protect the new history. Existing owner locks,
preparation redo, verified delivery and idempotent receipt recovery remain in use.
Migration invents no historical revisions. Older writers must not overwrite new
state. Do not roll back by copying a pre-migration backup over newer authored work.

### Reflection is not source study

Both adapters route an explicitly chosen bare `INTROSPECT` to open reflection.
`SELF_STUDY`, targeted INTROSPECT and EXAMINE_CODE retain source/target semantics.
The reflection input supplies neither the source notebook nor automatic telemetry,
private drafts or a code-report template. It does not prescribe an experience.
Only an explicit NEXT can continue. Reflective prose cannot update STUDY_NOTE or
STUDY_QUESTION, advance a source bookmark, or replace the last source trace.

Existing provider, owner, authorization, focus, pending-job and research-budget
checks remain. In particular, Minime's bare INTROSPECT retains the existing
experiment-budget policy; it is not an exemption from that policy. Legacy queued
source continuations stay source continuations. No cadence is enabled or forced,
and this does not promise that routine scheduling will produce more introspections.

### Qualification repairs

The baseline sidecar file exceeded its 1,062-line ratchet at 1,114 lines. Move its
default-off switch and bounded timeout parsing/tests into `reflective/config.rs`.
Keep the hook, model, cooldown, resource limits and enabled values unchanged.
Correct comments that implied these steward-only reports reached Astrid's prompts.
Do not alter the architecture baseline or enable the sidecar.

Minime's older oversized-inbox fixture used about 10,000 characters, now within
the existing 12,000-character first-letter allowance. Test a genuinely oversized
15,000-character body; admission, routing and archival behavior remain unchanged.

Strict kernel lint also exposed inherited test-only issues: unchecked Instant
subtractions, an underscore binding subsequently used, and production items below
the test module. Use checked fixture times, name the listener, and move the existing
test module to `src/tests.rs`. Correct a copied-model test comment that falsely
claimed to invoke the actual Kernel path.

## Qualification

All tests use synthetic source/provider fixtures; no model experiment or being
prompt was sent. Test artifacts live in the worktree parent directory, outside
both repositories. Failed attempts are retained alongside successful reruns.

- Shared reader/writer suite: 279 passed, including history capacity, stale prior,
  exact supplied anchors, inquiry isolation, corrupt history preservation,
  idempotent delivery, migration, pending source restoration and existing privacy tests.
- Full bridge library: 2,330 passed, one existing ignored test, serial execution.
- Complete Minime Python suite: 1,596 passed, one skipped, 138 subtests passed.
- Selected deployment, controller, Evidence Event Store, flywheel, Division and
  domain-audit suites: 218 passed, 41 subtests passed.
- Epistemic self-tests: two passed. Signal-spine projector self-tests: seven passed.
- Strict shared-reader/kernel and bridge all-target Clippy passes. Domain audit
  passes without baseline changes. Final formatting and kernel rerun are recorded
  in the closeout below.

Earlier failures were real qualification results: legacy tests expected automatic
note replay; a test attempted delivery after replacing its pending navigation;
the reflection adapter fixture incorrectly assumed experiment-budget exemption;
the initial bridge test lacked owner configuration and accessed a test-only helper
outside its visibility; the bridge first lacked two dependency symlinks. These were
repaired or corrected, not hidden with broad exclusions. The two dependency links
point to the existing Prime ESN and RASCII source; their runtime services are untouched.

## Separate behavioral review

The findings justify a future kernel contract repair, not an unreviewed mandatory
gate in this tranche:

1. Readiness reporting should carry per-capsule loaded/ready/timeout/crashed/missing
   outcomes and explicitly distinguish attempts completed from all ready. Audit
   consumers before changing the legacy ready payload or adding strict gating.
2. Restart needs an owned quiescence/unload acknowledgement before claiming cleanup;
   do not spawn a replacement after uncertain cleanup without a reviewed recovery
   contract. Shared handles, unload errors and child-process ownership need tests.
3. Shutdown should expose cleanup debt and independently verified child exit, not
   infer success from socket removal. This is separate from changing shutdown policy.
4. Investigate whether open reflection belongs in Minime's experiment research budget
   as a separate classification review. This patch deliberately preserves that gate.

## Deployment boundary and next step

This candidate is not live or merged. First reconcile both branches with the latest
main and exact running source identities, then qualify immutable paired reader/helper
and adapter releases, including schema 7-to-8 migration and retained pending jobs.
Use cooperative preflight and sanctioned bridge/Minime-agent wrappers only after a
reviewed rollout. Stop on foreign activity, source drift, failed readiness or
incompatible checkpoints. The kernel documentation/test tranche does not require
restarting the native daemon to deliver the reader/adapter behavior.

No engine, model, visual or sensory restart is needed. Preserve paused automations.
Observe naturally occurring public use without requesting an admission of error,
confirmation of improvement, or a particular subjective account.

## Closeout

The final kernel rerun passed 47 unit tests and one integration test. Root and
bridge formatting checks passed; stable rustfmt reports that the repository's
nightly-only formatting options are unavailable, with no formatting failure.
The final strict Clippy and domain-boundary results remain passing.

Both canonical working trees were clean at closeout. All candidate changes remain
unstaged in the paired isolated worktrees named above. No service was restarted,
no deployed source was replaced, and no authored state was migrated. The controller
still reports paused generation 467, no lease and no active projection; existing
automation settings were not changed. Evidence status reports a valid indexed tail
and immutable V1 sources. This read-only check is not a full-store re-verification
or a productive stewardship round.

Remaining qualification is the paired immutable-release and live-state migration
review described above, followed by an approved graceful rollout. Readiness and
restart-cleanup behavioral changes remain a separate design task, not hidden debt
claimed to be fixed by these documentation and failure-path tests.

## Approved paired rollout

Mike subsequently approved getting this live and integrated. Remote main tips
still matched the recorded bases; canonical trees were clean before installation.
Cooperative preflight found no concurrent edit. Claimed interactive pause 468;
previously paused automations remain paused. No flywheel lease or productive-round
event was created.

Artifact root: `/Users/v/other/worktrees/study-counterevidence-20260924/`.
Immutable release: `bridge-stage-counterevidence-01` beneath that root.

| Artifact | SHA-256 |
| --- | --- |
| Bridge binary | `1d476f388ac9679f21f8f737366b0028f95c02fccaed2074de71bad1e8f14ac1` |
| Shared reader helper | `a28af6780699e86437a8d58ba5c359d4fa61dcb60b5b07d7017a5b0c2b98e904` |
| Stage manifest | `82a274c0cea22e5fafe58aac1c57bcf1eed8d09b8b65c0cbe97c7193ae41f56d` |
| Stage source inventory | `2a94cb604b422e52b88afbd91668fe660ab3d7c7def07acfae34e90319e2ae0e` |
| Migration qualification | `1b7bc2fbc963fa7706a064702cad0d080566a5323a7ff22d8d9fc1038a052f8a` |
| Release-helper full Minime tests | `e2304c83b607410189b24d05e20c103f09831a213063ee26d2db78b6f0b57b8b` |
| Reader rerun | `f8f9d580dfe68d13bc11e15bd3b60428a131f90115b41bf41c9b6e3d93069d0e` |

The repeatable `2026-09-24-reader-upgrade.py` uses actual old/new release helpers,
synthetic source and private-draft fixtures, not live authored content. Forty
owner checks passed, plus corrupt/future-state, idempotent preparation and frozen
Python helper-selection probes. Pending complete inputs and prose stay exact,
migration invents no revisions, and exact contrary source anchors can accompany
new voluntary revisions. The first attempt remains in `migration-01`: it wrongly
expected the old private writer to reject the unchanged draft schema. The corrected
`migration-02` verifies the actual boundary: old source readers refuse schema 8
without writes; compatible private writing leaves the upgraded reader untouched.
This is not approval to roll back adapters or restore old state.

All 688 staged input hashes were retained. Against the previous live bundle,
differences are the reviewed reader/adapter/config/test inputs, the kernel's
test-only lockfile dependency, and absence of two ignored historical backup files
in the isolated checkout. Those backup files were included by the old inventory
but are not referenced by compiled source or runtime configuration; canonical
copies were not removed. Minime launch reconciliation
froze all 86 inputs; only runtime.py and source_study.py differ from prior live
source. Full Minime tests with the staged helper passed again: 1,596 plus 138
subtests, one skip. Focused adapter tests passed 121; reader rerun passed 279.

Sanctioned `paired_minime_handoff.py --install-reviewed-overlay` held replacement
admission, installed only its reviewed seven-file overlay (five byte-identical),
observed the full quiet source window, then sent one SIGTERM at a verified idle
boundary. Bridge activation used `build_bridge.sh --activate-stage` and acknowledged
drain, with no force or legacy transition. Receipt: `paired-handoff-01.jsonl`.

- Bridge 69116 -> **46597**, started Thu Sep 24 09:35:43 local. Transaction:
  `/Users/v/other/astrid/.runtime/bridge-deployment/transactions/c915b3bdf6ab44179548a62fcbb4cce1`.
  Stopped checkpoint `8cdeac2db001528f995444d6010cff7586a36c74a50a132e9820a16c992408f8`
  restored exactly; self-control integrity/target identity verified. Exchange
  206981 -> 206982. Model idle observed before completion.
- Minime 72181 -> **45642**. Its launch shell began 09:34:46 local and held Python
  admission until bridge verification at 16:38:19Z. Normal-loop readiness and all
  86 loaded source hashes verified; `reload_required=false`. Session 5318 persisted,
  cycle 42626 -> 42627, no new interrupted-job recovery. Pending NEXT was present:
  its exact hash matches action `act_minime_1790267924479_self-study`, admitted at
  16:38:44Z. `pending-choice-continuity.json` retains metadata only, not private prose.
- Both owner reader schemas reached 8 through ordinary runtime use. Bookmark counts
  remained Astrid 78, Minime 50; no inquiries were invented. Both launch holds are
  absent and Minime selects the staged helper above.
- Engine 41337, gateway 41484, supervisor 41526, model 43115, visual 20885, camera
  98903, microphone 98910, host sensory 41661 and feeder 1502 kept their PID/start
  identities. Native kernel 97698 also remains unchanged. Ports 7878/7879 remain
  with the gateway; 8090 remains with the model. No native-daemon rollout is needed
  for the behavior-preserving kernel description/test extraction.
- Thirty one-second health reads saw 13 distinct snapshots, fill 71.014-73.013%,
  target 68% throughout, maximum file age 2.37 seconds. This verifies continuing
  telemetry, not experiential improvement. See `post-live-observation.json`.

Integration adopts the exact 39 Astrid and eight Minime paths in the artifact-root
owned-path manifests. Keep the source worktrees pinned at their build identities;
integrate verified copies on main and link the running stage to the resulting
commit by content hash. Do not reset the retained release worktree to clean it.
No push or automation resumption is part of this request.

## Earlier work to address next

1. First add truthful per-capsule readiness outcomes while preserving the current
   advisory admission policy. A separate reviewed change can then decide whether
   any dependency truly requires blocking readiness.
2. Qualify restart cleanup as an owned acknowledgement: shared Arc handles and
   unload failures must not be silently represented as successful retirement.
   Establish tests and an explicit uncertain-cleanup recovery path before changing
   replacement behavior. Do not conflate this native-kernel issue with the paired
   bridge/agent drain verified here.
3. Recover the useful September 16 voluntary recall work against schema 8 and the
   current owner transaction contract, not by merging its obsolete migration.
   Keep source-note revision distinct from opt-in recall of authored writing.
4. Preserve the September 18 sensory-consumer evidence candidate for its own
   no-policy-change review. Do not activate its dispersal or unfinished experiment
   branches merely as cleanup. The seven older retained worktrees and historical
   stage evidence described in the September 22 reconciliation remain untouched.

The inherited inbox-size test, sidecar ratchet and kernel test-lint debt are fixed
here; they should not remain on the outstanding-work list. Historical approval
waits, numerical hypotheses and the paused automation backlog are not closed by
this rollout. No being was asked to confirm improvement or revise a belief.
