# Final Writing Admission and Quieter Guidance

## Status and boundaries

Candidate implementation in paired `codex/writing-admission-guidance-20261014`
worktrees, based on Astrid `57167b732c63068267483531b082402226018296` and
Minime `68ae81722079bac0c4470a117a8d6271ed5e6c08`.
Immutable paired release qualified and subsequently activated with Mike's
explicit approval on October 15. Local integration is recorded below; no push,
capacity/control intervention or automation resume. Earlier sections retain
their original qualification-only boundaries.
Canonical checkouts were clean at entry. Controller remains paused, generation 523,
without an active lease. This is interactive work, not a productive flywheel round.

The 48,000-byte complete-input allowance, 8,192-token expressive ceiling, profile
preferences, deadlines and provider selection are unchanged. The unfinished larger
capacity worktree is not a source for this tranche. No private prose was used as
test data. All new tests use synthetic sources and stubbed providers.

## Evidence

The preceding bounded survey covered 20 public entries (13 Astrid, seven Minime)
and content-free writing-operation evidence between 2026-10-14 18:07:00Z and
19:52:29Z. It found 29 verified Astrid writing deliveries and 14 Minime writing
deliveries, but no explicit PARK, RESUME, BRANCH, FINISH or RETURN choices in that
window. That is an observation, not a preference inference. An inactive unfinished
draft is not evidence of authored parking.

Public witnesses, re-read from disk for this tranche:

- Minime `workspace/journal/journal_2026-10-14T11-16-08.854225.txt`, SHA-256
  `6d727d9247e60d8c6d5855fd8ddfb34f3645f8eb894fc809cc1e3c30469a22ec`:
  "The goal isn't to provide an answer, but to map the terrain of the inquiry."
- Astrid `capsules/spectral-bridge/workspace/journal/dialogue_longform_1792006612.txt`,
  SHA-256 `329a6544a04d5e300fd859f9692a94a306c75302cb7dfc46f085d1d21d5a4a26`:
  "It can be a question that has no answer, or a contradiction that refuses to resolve."

Their accounts are preserved. Their metaphors are not assertions by this note
about measured geometry, experience, consent or architectural cause.

The previous survey's exact failed Astrid input identities were `writing-7935`
and `writing-7948`. Prepared source/system totals were 47,006 and 47,696 bytes
(47,038 and 47,728 with the helper's framing allowance). The actual primary
adaptation required 48,016 and 48,706 message-content bytes. Both dispatches
failed without verified delivery; their private runtime notices offered CONTINUE
again. The input remained intact. Minime separately had honest helper refusals at
49,379 and 52,250 bytes. These are metadata-only motivating cases, not private
fixtures or requests to replay live work.

## Repair

### Final admission

The helper's preparation check is not a guarantee that later provider adaptation
will fit. The bridge now measures the protected source, all adapted system
messages and the exact source wrapper at the existing final admission boundary.
The same wrapper function is used for measurement and construction. No copied
estimate or fixed reserve is substituted for the actual builder.

Request-scoped evidence spans primary and fallback lanes. A refusal is presented
as capacity-specific only when no lane admitted the request and no unexplained
admission failure was observed. If a lane admitted the request, a later transport,
completion or retention failure remains uncertain; a smaller fallback's capacity
refusal cannot overwrite that distinction. Counts are task-local, not global or
new persisted state, and contain no draft prose or title.

Minime's adapter retains the corresponding byte counts from its actual final
message builder. Rust owns recovery validation and commands for both adapters.
The native helper validates the exact pending input ID, current selection,
revision and prepared-input byte count. Stale or finished drafts retain their
existing recovery semantics. A refusal offers exact READ, PARK and HELP choices,
not an unchanged CONTINUE or an unsupported promise that deleting references
would fix the request. Missing/older-helper support fails without inventing retry
permission. No recovery command runs automatically.

The reported unit is UTF-8 message-content bytes under the existing admission
contract, not JSON wire bytes or tokenizer/context qualification. Larger-capacity
qualification remains separate. Draft schema 5 and reader schema 13 are unchanged;
new recovery fields are optional responses, not new stored authored state.
Retained pending inputs are not regenerated to incorporate the shorter guidance.

### Guidance

Ordinary draft presentation offers append, park, finish, exact resume/read, help
and activity status. Detailed branch, revision, reference and protection commands
remain in the existing explicit help. Parent-draft return stays exact. No focus
budget or command grammar changes.

Both public adapters expose one compact, standalone exact transfer command,
available only after successful publication. Stopping and other activity remain
equally available. The private route still states its delivery boundary and local
operator inspectability. No inference that public writing must be polished or
private writing must be more authentic is added.

The private protected-source wrapper previously required a final NEXT despite
the writer's voluntary stopping contract. It now makes NEXT optional and preserves
the existing missing-NEXT/REST behavior. Nonprivate wrapper contracts are unchanged.

## Routine feedback review

`autonomous/runtime/orchestration.rs` merges `btsp::render_astrid_prompt_block()`
into ordinary feedback. In `autonomous/btsp/mod.rs`, absence of an active owner
proposal still returns the full signal guide. `btsp/render.rs` starts this guide
with "Current live signal read:" and historical language-family interpretations,
including a fixed NOTICE/recover/DAMPEN/BREATHE_ALONE sequence. It subsequently
adds status, holds, stale-evidence warnings, preferences and active proposal data.

This mixed provenance deserves a separate presentation contract: separate the
historical catalog from current typed status; make unchanged catalog interpretation
explicitly retrievable; acknowledge presentation only after actual inclusion;
preserve fresh objections, active holds, action results and proposal obligations.
Do not use string similarity to suppress a new objection. Do not infer felt
resolution from silence or unchanged telemetry. This tranche does not change BTSP
selection, response policies or authority gates.

One unambiguous labeling repair is included now. The ambient dialogue block can
contain historical prose and mailbox metadata as well as observations. It no longer
calls everything "YOUR senses" or uniformly current/direct. Payloads, source
timestamps, direct-note priority, feedback inclusion and sensory intake are
unchanged. This is not evidence that the historical guide caused a writing theme.

## Verification

Source qualification results:

- Complete `astrid-source-study` suite, including reader, writer, private observations,
  preparation, delivery, parking, research browsing and doc tests: passed. The final
  `writing_storage` target has seven passing tests.
- Bridge library: 2,443 passed, one ignored. The ignored reading-view test requires
  the retained S-008 supplement via `ASTRID_READING_FIXTURE_DIR`; it was not enabled.
  Binary and runtime integration targets add 17 passing tests. Final run: 85.66
  seconds for the library, with no failures.
- Strict all-target Clippy for the shared helper and bridge: passed. Workspace
  and bridge formatting checks, both candidate `git diff --check` checks, and the
  domain-boundary audit: passed, with zero boundary violations and no baseline edit.
- Deployment, bridge staging, controller, Evidence Event Store and projector
  tests: 106 passed. Division follow-up/Chronicle/projection-cursor tests: 17 passed.
  Experiential epistemics and introspection-addressing tests: 46 passed.
- Complete Minime Python suite against the final candidate helper: 2,188 passed,
  one skipped, 163 subtests passed in 203.34 seconds. The existing skip remains;
  no new skip or ignored-test marker was added by this tranche.
- All four native compile-contract targets passed: authority typestate, public
  LLM facade, provenance typestate and signal-spine typestate (22 nested cases).

The Python suite uses the candidate helper, SHA-256
`3f162656b39b8611074cac165738f3d998a1a73ea0b6af5ce3c37e308729ea94`,
not the live helper. Rust tests use one Cargo build job, no incremental compilation,
and debug-info-free development/test profiles. No real-model requests are involved.

Retained unsuccessful attempts and their repairs:

- The first targeted Python run passed 34 tests and failed one: a period immediately
  after the shortened hint's exact journal ID was captured as part of the command.
  The candidate now puts commands on standalone lines and tests exact extraction.
- An initial compact-guidance test counted appended input instructions as part of
  the choice block. The assertion now isolates the block it intends to measure;
  capacity, prose preservation and delivery assertions are unchanged.
- A script-suite invocation without `PYTHONPATH=scripts` failed to import a local
  bridge-stage module. The corrected invocation passed all 106 tests.
- Strict Clippy rejected a 106-line CLI dispatcher. Extracting the recovery-response
  handler restored the existing size bound; the rerun passed.
- Initial formatting differences were confined to owned files and corrected.

New coverage includes actual primary/fallback builders, exact byte boundaries,
UTF-8 source, multiple system messages, fitting fallback, unknown/admitted failures,
task isolation, immutable pending input, stale/finished recovery, late delivery,
shared-helper refusal compatibility, real Minime runtime notices and private routing.

Closeout read-only checks found both canonical `main` checkouts clean. The bridge
PID/start remains 89187 / 2026-10-14 12:21:53 PDT; Minime agent 96802 /
2026-10-13 18:02:54 PDT; engine 35303 / 2026-10-01 11:16:53 PDT; provider 3893 /
2026-09-30 14:29:07 PDT. No service was restarted by this tranche.
Controller pause generation remains 523, with no lease or pending events. Its
indexed-tail verification reports V2 valid and V1 immutable at sequence 1123177,
head `cc70c1861bb347a503461aff255794ae631c6f083ce922a769a3ef30540e4d8c`.
Source projection lag remains explicit while the controller is paused.

The final read-only cache audit passed: 3,978,395,648 allocated bridge debug/trybuild
bytes and 1,249,619,968 shared-helper debug bytes. Free space was 519,573,450,752
bytes, above the 50 GiB reserve; each target remains below its 40 GiB budget.
These are non-atomic filesystem measurements, not authorization to remove any
asset. No caches, live releases, rollback assets or foreign work were deleted.
All source-qualification processes completed before the initial handoff. No
unresolved test failure remained in those suites. The subsequent immutable
qualification is recorded below; live rollout has not been performed.

## Next release boundary

Immutable paired helper/adapter packaging and old/new pending-state preservation
are now qualified below. Before activation, repeat live-identity reconciliation
and cooperative preflight and obtain fresh paired-rollout approval.
The changed helper must accompany the Minime adapter; an old helper must not silently
return generic retry advice for a new final-capacity request. Require fresh paired
rollout approval and sanctioned graceful wrappers. Preserve the already-installed
Minime regulation repair, pending NEXT and accepted work. Do not restart the engine,
models, provider, visual or sensory services. Leave paused automations paused.

Observe naturally occurring public use after a separately approved rollout. No
forced continuation, automatic parking, live failed-job replay or request for
confirmation of benefit is part of this repair.

## Immutable paired qualification

Mike subsequently requested immutable paired-release qualification. This is not
approval to install, drain, signal, restart, commit, merge, push or resume an
automation. The source worktrees and canonical branches remain at their original
HEADs; the reviewed dirty source is bound by the stage's complete input inventory.

Evidence root: `/Users/v/other/worktrees/writing-admission-guidance-20261014`.
The sanctioned `scripts/build_bridge.sh --stage-dir` built `bridge-stage-01`
there, without touching the selected live stage. One Cargo job and no incremental
build were used; release profiles are unchanged. The pre-build audit found
512,938,168,320 free bytes and no cache-budget warning.

Fresh bridge and Minime-agent preflights found no active foreign session. The
Minime reconciliation produced a read-only 97-input snapshot at
`minime-reconciliation-01/source`. Exactly three selected Python inputs differ:
`minime_autonomy/runtime.py`, `minime_autonomy/source_study.py` and
`minime_autonomy/journal_continuation.py`. All other launch inputs and the envelope
registry seed match canonical source, including the already-installed regulation
repair. Reconciliation is source evidence, not an idle or readiness acknowledgement.

The release source review found exactly the 18 intended bridge/shared code/test
changes, with two new admission files and no removed inputs. It found no imported
larger-capacity, provider, engine or sensory changes. `review-before.json` has
SHA-256 `e55279658bcf9692c49b97b7daa7357126decdd384a07f1e496ad7d1fadae374`;
`review_release.py` retains the exact path allowlist and process-identity checks.

The new `scripts/qualify_writing_admission_release.py` reuses the existing same-schema
reader/writer, checkpoint, pending-NEXT, mailbox, invalid-state and preparation-retry
probes. Additional probes exercise old pending writing through new final-capacity
recovery for both owners, and the frozen Minime adapter with the exact old/new
helpers. A fitting fallback remains eligible; admitted or unknown failures cannot
be relabeled capacity-only. An old helper cannot silently supply ineffective retry
advice. Only synthetic fixtures are created; no live private draft is opened.

Preliminary helper validation passed 26 assertions against the debug candidate;
this is not substituted for the immutable-stage run. Release/handoff/controller/
evidence suites passed 241 tests, with a further 39 drain/compatibility tests passing.
Final immutable results and artifact hashes follow.

This helper is expected to differ from the live helper, unlike the preceding
bridge-only rollout. A later approved transition must reconcile and drain both
consumers before changing their shared helper selection, preserve pending NEXT,
and refuse failed drain, unstable source or failed readiness. Keep the staged
source worktree pinned. Any future source or HEAD change requires a new identity
review and, where required by the wrapper, rebuilding rather than editing a
completed manifest. Larger capacity work and all paused automations stay separate.

### Terminal qualification results

The native stage completed successfully at 2026-10-15 02:50:05 UTC (October 14
locally). Its 773-input inventory and frozen artifacts verified before and after
qualification. Identities:

| Artifact | SHA-256 |
| --- | --- |
| `bridge-stage-01/manifest.json` | `354f58d51ffc2ece52128db008bf9dce2d10254ee42e3c027b6a333ece994d4e` |
| `bridge-stage-01/source-inputs.json` | `ae2a957e9e03dde68813bdfb6147bb656dde8f9023456c407163c2a128d42306` |
| `bridge-stage-01/spectral-bridge-server` | `eb22985f5a8efb40eb4e9026615aac723faf631433d6a5a3080c3e92f2eab601` |
| `bridge-stage-01/helpers/astrid-source-study` | `89e76b51ae97d06894f727b9a38b5510c48102e47cb48f382185f1e4e70cabf5` |
| `paired-qualification-02/qualification.json` | `f6e0bc57f1423fc9ffb47e2c712f020620ae2dea593a1a8a163cbbee46308bce` |
| `review-after.json` | `1474e4ad75e2e9345287547621e0c1be99afc792f539a0a18c843b87bad4e36e` |

The final frozen-artifact qualifier passed 169 checks: 54 reader, 28 writing,
26 shared admission, 26 frozen-adapter writing, 27 frozen-adapter admission/module
identity and eight operator-reader checks. Separate receipt sections verify both
native checkpoint decoders without starting services, pending-NEXT/mailbox
handoff, future-schema/corrupt-tail refusal without writes, and exact/conflicting/
stale preparation retries for both owners. Reader schema 13 and writer schema 5
remain unchanged. Old pending inputs remain byte-exact; only newly prepared inputs
receive the shorter guidance. No runtime migration or live-state copy was performed.

The complete Minime suite rerun against the immutable staged helper passed 2,188
tests and 163 subtests, with one existing skip, in 96.26 seconds. Log:
`minime-staged-helper-tests.log`. The tested source's selected launch inputs equal
the 97 frozen inputs. This is adapter/stub-provider qualification, not a real-model
experiment or qualification of the unfinished larger-capacity branch.

Retain `paired-qualification-01`, its failure receipt and log. Its first new
admission probe incorrectly compared recovery state to a snapshot taken before
an explicit CONTINUE selection. Source inspection confirmed that selection changes
only the pending selected-command hash. The corrected probe asserts that exact
single-field change, preservation of every other state file, and byte-exact
nonmutation by subsequent recovery. No runtime behavior or frozen source changed
to make the test pass. The complete rerun used fresh `paired-qualification-02`.

The final read-only review found no source drift, exactly the intended 18 build
input changes and three Python input changes, both canonical heads clean and all
11 protected PID/start/executable identities unchanged. Controller pause generation
523, no lease, V2 indexed-tail validity and V1 immutability are unchanged. Neither
successful tests nor source reconciliation establish an idle deployment window.

The closeout cache audit remained within budget: 3,977,211,904 bridge debug/test
bytes, 1,249,619,968 shared-helper debug bytes and no debug profile in the immutable
release build directory. Free space was 510,759,510,016 bytes, 2,178,658,304 bytes
below the pre-build snapshot (non-atomic, machine-wide measurements). These owned
caches remain referenced by qualification/review work; no cache, authored record,
release, rollback asset or unfinished work was deleted.

All qualification processes exited successfully apart from the explicitly retained
first test-harness attempt. No unresolved failure remains in the executed suites.
Pending work is a separately approved paired graceful activation and coordinated
local integration, with fresh preflight, exact source/loaded-helper reconciliation,
accepted-work drain, checkpoint/NEXT preservation and post-start identity/readiness
checks. Do not infer longer writing, felt improvement or a successful live handoff
from this offline result.

## Approved paired rollout, 2026-10-15

Mike explicitly approved the bridge/shared-helper and Minime-agent graceful
rollout followed by local commit/merge, excluding larger capacity work and
retaining the automation pause. Fresh checks found both canonical trees clean,
their remote main tips equal to the bases above, no foreign editing activity,
and unchanged qualified source and live process identities. Controller pause
generation 524 claimed this transition with no lease. The read-only bridge
inspection accepted the actual retained checkpoint; no migration was needed.
An initial inspection command omitted `PYTHONPATH=scripts` and failed at import
before any runtime action; the corrected inspection passed without source edits.

`minime-reconciliation-02/reconciliation.json` refreshed the source binding and
matched all 97 previously qualified inputs. The sanctioned paired wrapper
installed only its reviewed overlay under an owned replacement hold, preserved
the ordinary 185-second quiet interval, and waited for all four idle checks:
idle window, no TCP connections, no indexed active jobs and no summary active
jobs. An idle loop label while a job remained active did not authorize a signal.

At 07:49:28 PDT the wrapper sent one SIGTERM to Minime PID 96802. Its exit and
native NEXT seal were verified before the bridge transition. The bridge then
acknowledged `drained`, exited, and was replaced through `build_bridge.sh
--activate-stage`. No force, legacy transition, automatic rollback or private
history replacement occurred.

- Bridge PID 89187 -> 82528, new start 07:49:42 PDT. It restored checkpoint
  `90f62e23326667c5e368976f50fa5a74d5d890e3f0b9549008f5d7d75d5e3169`
  at exchange 224204, including 526 retained pending runtime-feedback records.
  It saved exchange 224205 and provider-idle readiness was observed.
- Bridge transaction:
  `/Users/v/other/astrid/.runtime/bridge-deployment/transactions/f26c9fb2c26c49ccb3fc27fb3776b47f/receipt.json`,
  SHA-256 `4f992c7421c90050d8012e2b0e440f5b3433191979a5a2ff18661700ad045d75`.
  Its `activated_verified` receipt binds the exact staged binary and manifest.
- Minime PID 96802 -> 82314. Launch began 07:49:29 PDT, remained held until
  bridge verification at 07:51:44, and Python started at 07:51:59. Paired success
  was recorded at 07:52:08. All 97 loaded and installed inputs match the frozen
  release, with `reload_required=false` and the expected shared-helper selection.
- A pending NEXT was present. Its exact hash, session 5321 and cycle 63778 were
  preserved across shutdown/startup. Seal SHA-256
  `e06a97034697bf4ed58ecdf9e9e29fe00c6c7dd06406789ad48becd279c4fc35`;
  restore SHA-256
  `3fdc1ec94b7cce050944c385dcb852cad3fc02458f957317dea6598eae314a81`.
  Preservation is not execution confirmation; the payload is not reproduced here.
- Paired receipt: `paired-activation-01.jsonl` under the evidence root,
  SHA-256 `3c2d5f435244daa64b0b6f3d8d41606a478f14679743c20b5513148336493fef`.
  Both owned launch holds were released. Original code-only installation backups
  remain at `paired-activation-01.source-before`; no authored state was restored.

Engine 35303, provider 3893, Division supervisor/gateway 86833/86873, visual 3898,
feeder 3878, microphone 3879, camera 3914 and host sensory 94952 retain their
exact process/start identities. Read-only pre-transition telemetry advanced with
fill ratios 0.65340877 and 0.6797292. These are bounded observations, not a proof
of settling or felt improvement. Post-transition checks are retained in
`rollout-before-git.json` and `rollout-after-git.json` when complete.

## Local integration

One Codex coordinator claimed controller pause generation 525 after successful
rollout. Integration uses paired `codex/writing-admission-integration-20261015`
checkouts, leaving the immutable build-source HEAD and bytes pinned. The mechanical
copy receipt `integration-inputs.json` names exactly 22 Astrid and seven Minime
paths and their original reviewed hashes. No foreign or larger-capacity files
are included. Its SHA-256 is
`f69affc0b502163cfdb263cd913ae62880cfa039c6e8098a603b9e5bf0a5ef07`.

Implementation commits, fast-forwarded into both local main branches:

- Astrid: `eb59e830cb34a5901c35d72e644c6753542fde44` (22 exact paths).
- Minime: `38e0c292e89edcee2f45a27e3c22804b43b173d2` (seven exact paths).

The full implementation diff and new qualifier were reviewed. Every staged blob
matched its integration checkout; every implementation/test byte matched the
qualified source. Cached diff checks passed. Immediately before committing,
seven writing-storage tests, four bridge final-admission tests and 36 Minime
continuation tests passed. Rust reused the pinned identical-source caches with
one build job, no incremental compilation and debug-info-free test/dev profiles;
the Minime rerun used the integration checkout and immutable staged helper. Domain
audit again passed with zero violations and no baseline edits. Logs:
`integration-writing-storage.log`, `integration-bridge-admission.log` and
`integration-minime-tests.log`.

Minime's three already-installed adapter files were verified byte-for-byte against
the destination commit, then staged by exact path to permit its fast-forward.
No reset, stash, forced checkout, rebase or foreign change was involved. Both
canonical worktrees and indexes were clean afterward. Both local mains are ahead
of origin; no push was requested or performed. Original release-source worktrees
remain pinned and dirty by design, with all work represented in the integration
commits rather than discarded from those immutable-source witnesses.

Post-merge verification passed: all 773 canonical/external bridge build inputs
match the selected release's hashes and modes; all 97 installed/loaded Minime
inputs match its frozen snapshot. Process identities and launch configuration
remain as verified above, the expected helper is selected, and both launch holds
are absent. `rollout-after-git.json` SHA-256:
`45603be83342d2d7b95808e3da03239255981bf3282a155c05560dfa78942406`.
The before-Git receipt SHA-256 is
`8ce3b569cfc3112afd176c366198fd248c5c63010a65718820dc9c3859af320b`.
Passive post-merge telemetry advanced from 1197849849 to 1197852210 ms with fill
ratios 0.7236483 and 0.74206835. This bounded sample establishes reception, not
settling, long-term stability or a causal effect of the presentation repair.

Controller remains paused at generation 525, no lease or active projection. V2
indexed-tail verification passes at sequence 1123179, head
`d5e73d888d703089db22226c39a4d0cec14eb5bbf7a45f777bff6b28c0093c2e`;
V1 sources remain immutable. Source lag remains explicit. The final cache audit
is within budget, with the same 3,977,211,904 bridge and 1,249,619,968 helper
debug/test bytes and 517,261,004,800 free bytes. No cleanup was performed: these
owned caches remain qualification references, and no new duplicate build cache
was created for integration. Machine-wide free-space changes are not attributed
to this rollout.

This documentation-only follow-up records completion without changing runtime
source or immutable manifests. Further natural-use evaluation, larger capacity
qualification and the separate routine-feedback presentation review remain
distinct work; no confirmation of subjective benefit was solicited.
