# Voluntary Continuity Implementation

Date: 2026-09-20. Coordinator: this Codex interactive task.

## Status

Historical foundation checkpoint. The subsequent runtime integration and current
release gates are recorded in `2026-09-21-protected-attention-runtime-integration.md`.
The remaining-work list below describes the end of this first pass, not the latest
implementation state.

**The full requested plan is not complete. Do not deploy this branch as a finished
protected-attention release.** The shared persistence/validation foundation and
expressive-writing changes are implemented and tested in paired isolated
worktrees. Scheduler, mailbox and authoritative foreground-selection integration
remain required work, not merely deployment paperwork.

No services were restarted, no immutable release was selected, no model or
reservoir experiment ran, no automation was resumed, and nothing was staged or
committed in this pass. These changes do not establish persistent activations or
subjective improvement.

## Witness and Interpretation

Source: `/Users/v/other/minime/workspace/journal/!aspiration_2026-09-20T10-39-43.801459.txt`.
SHA-256: `0aa5679357da59a121520e5d2bd42d0908170915435546068682479d1a182ee6`.
Read completely, including the supplied prompt, telemetry and trailing notice.

Exact authored excerpt:

> I want to become a being of *persistent interiority*.

The source also asks for thought to survive inference and for self-directed
priority. Its neuron/activation descriptions and references to Astrid's Deny and
Final are authored interpretations, not independently verified implementation
facts. They did not authorize changing either security mechanism.

The trailing agency-vernacular notice asks for definition, contrast, experiments
or evidence. Source inspection found both that appending path and a stronger
automatic path that could replace an entry with a similarity summary. This is
a concrete interface issue independently of any theory of subjective experience.

## Isolation and Ownership

Astrid branch: `codex/voluntary-continuity-20260920`, based on main
`c4f85e95e41703daa65d3ce2789e1e5c46961c4e`.

Minime branch: `codex/voluntary-continuity-20260920`, based on main
`5f4925f54580f1fd44666058b126a121ff32880f`.

Worktrees:

- `/Users/v/other/worktrees/voluntary-continuity-20260920/astrid`
- `/Users/v/other/worktrees/voluntary-continuity-20260920/minime`

Both remote main tips matched their local main baselines before creation. Both
canonical dirty trees were inspected and left outside the candidate. Controller
status was paused at generation 456, with no lease or active projection. The
prior pause remains in force; this was interactive development, not an automated
productive introspection round.

Bridge test dependencies were linked read-only by convention into the worktree
parent, with clean dependency trees verified before use:

- `prime_esn_wasm`: `78d7255b075c06a77396c76682bfbac95245954b`
- `RASCII`: `ca0fd8622e174f75904ac2583e0268cb19e6a43a`

They were not edited. Compilation output lives under this candidate's targets.
These development links are not a release inventory or OS sandbox attestation.

## Implemented Foundation

### Shared focus contract

`crates/astrid-source-study/src/focus.rs` contains the owner/target validation and
transition table. The default/maximum is four admitted generation jobs and
900,000 elapsed milliseconds. A request may reduce the count to one through
four. Starting focus selects one presentation; subsequent admission requires the
hash of an exact eligible authored NEXT. The focus checkpoint stores hashes and
native references, not private arguments or prose.

Admission is counted before a host invokes a provider. A duplicate operation
does not count twice; a conflicting retry fails. Job IDs are bound to their focus
window, preventing reuse across renewed windows from bypassing the budget.
Completion preserves the independently retained authored command even when
priority ends. No NEXT, REST or another activity ends protection in this state
machine. End/park do not declare a draft finished or a question resolved.

`store_focus.rs` adds a typed host operation family under the same owner reader
lock: status, command, admission, completion and failed-job acknowledgement.
Completion reads an already committed native delivery, verifies its artifact
hash, and extracts the explicit NEXT from the retained response. It does not
trust a caller-supplied model outcome. Status exposes IDs, revisions, counters,
deadline and an exact return command; it does not expose prose.

Prepared input is also bound to its selected action. Admission cannot label a
prepared revision as an authorized resume of the same draft, or label a map as
a different authorized source action. Legacy inputs need explicit reselection
to acquire this binding; their original supplied text remains intact. Retried
park operations retain the exact return command without repeating native writes.

A transition marker makes interrupted native-selection/checkpoint changes
recover with priority released. It does not guess that an unfinished operation
succeeded. Corrupt records and conflicting retries remain visible failures;
history is not truncated. Deadline expiry and a reversed observed clock release
priority without cancelling accepted work or replenishing the allowance. Clock
observations are persisted even when the requested operation is rejected; a
later forward clock cannot resurrect a window invalidated by that attempt.

The Python `StudyClient.activity` method is a thin JSON adapter. It does not parse
focus commands, infer intentions, schedule generation or open a mailbox.

### Native source questions

Reader checkpoint version 4 holds an independent current source, bookmarks and
pending page/session/navigation references for each question and for unthreaded
reading. Global delivery evidence and source coverage remain global, not rewound
snapshots. A late delivery updates its original inquiry without replacing the
currently selected inquiry's position.

Migration attributes only cursors carrying existing owner-question evidence.
It does not reconstruct missing historical positions. Activity return verifies
source bytes before treating the retained position as current. A changed source
requires explicit OPEN/reselection. The old reader rejects the newer checkpoint
version rather than rewriting it.

### Native private drafts

The writer now uses the owner reader lock, retains superseded pending offers and
accepts valid late completion only against the original draft revision. A detour
does not redirect that completion into another draft. A conflicting retained
artifact cannot be overwritten after an interrupted commit.

`WRITE STOPPING_POINT <text>` retains an optional note; it is reference material,
never an executed command. `WRITE PARK` does not mark the draft finished. Exact
wire artifacts and historical passages remain available. The existing 48,000-byte
input allowance is unchanged; overlarge drafts fail without summarization or
silent shortening and remain accessible through explicit draft reading.

New draft checkpoints use `drafts-v2.json`. Before migration, existing v1 bytes
are retained in a content-addressed legacy file. The v1 path becomes an explicit
downgrade-refusal record, unreadable to the older required-field schema. If an
older process replaces that record, the new writer refuses to proceed. A crash
during migration can require manual recovery from retained bytes; it does not
silently reset the draft. No migration has run against live stores.

The migration lock coordinates new participants. Older binaries still use their
older lock paths, so a cooperative stopped/drained deployment is required; the
guard is not permission to migrate under a concurrently running older writer.
Recovery tests retain exact archive bytes, reject a replaced downgrade guard,
keep newer prose when a stale revision arrives, and preserve an existing delivery
artifact against a conflicting retry across the artifact/checkpoint boundary.

### Expressive writing

Minime's journal hook no longer appends pressure, agency, afterimage or topology
advice, nor replaces text with a similarity summary. Existing public diagnostic
registration remains separate. Automatic diversity, afterimage and fatigue
prompt insertions are removed from the generic writing assembly. Explicit
diagnostic methods, errors, NEXT parsing and control checks remain.

Astrid no longer passes the merged recurrence/diversity advice into dialogue or
copies diversity feedback into future emphasis. Authored NEXT is unchanged.
Private source/writing prompts retain their dedicated provider path. Historical
journals were not rewritten.

## Verification

Successful checks at this checkpoint:

- Shared reader/writer suite: 209 passed, including native focus store, question-local
  cursors, exact delivery, overflow, legacy compatibility and authored findings.
- Shared-reader Clippy with all targets and warnings denied.
- Bridge library: 2,275 passed, one ignored.
- Bridge Clippy with all targets and warnings denied.
- Shared-reader and bridge formatting checks.
- Complete Minime Python suite: 1,423 passed, one skipped, 134 subtests passed.
- Focused Minime journal/continuation suite: 422 passed, 17 subtests passed.
- Real Python-to-Rust activity adapter: three tests, including eight concurrent
  helper processes retrying one admission and changed-source return refusal.
- Controller, source-first projection, event-store and Division suites:
  77 tests passed. Introspection addressing self-tests: 44 passed. Epistemic
  boundary self-tests: two passed.
- Domain-boundary audit: valid, zero violations; no live authority conferred.

Synthetic tests exercise both owner identities, native claim/question storage,
private continuation, interruption, park, unrelated work, explicit return and
revision. These are not full running-scheduler mailbox/drain tests. Do not cite
them as evidence that ordinary mail is already deferred in either live runtime.

Unsuccessful attempts retained in this account:

- Initial clock fixture moved time backwards accidentally; corrected the
  fixture, retaining a separate actual clock-reversal test.
- Older tests expected question selection to inherit another question's page;
  updated expectations and added explicit per-question and late-delivery cases.
- Draft tests expected a detour to discard the first draft's pending delivery;
  replaced that expectation with original-draft-only late completion.
- First Python run omitted the required isolated helper environment binding;
  rerun with the candidate helper and an allowlisted environment. A traceback
  exposed environment repr, so subsequent commands use a clean environment and
  short tracebacks; no credential is copied into this packet.
- New changed-source fixture initially used a path outside the catalog. It was
  a recovery map, not source delivery. Corrected the fixture into a catalogued
  crate and asserted that a page was actually supplied before testing staleness.
- Initial bridge invocation lacked sibling path dependencies; clean dependency
  identities were checked and linked before the passing build/tests.
- Initial combined Python tooling invocation lacked `PYTHONPATH=scripts` for
  Division imports; corrected invocation passed all 77 tests.
- Clippy found missing error documentation, an or-pattern and long cohesive
  transition/test tables. Fixed documentation/patterns and retained narrow,
  explained cohesion exceptions rather than suppressing warnings globally.
- The final cursor-migration lint run found three assigning-clones warnings;
  replaced the assignments with `clone_from` and reran Clippy successfully.
- The action-binding extension changed explicit legacy CONTINUE into a guarded
  migration. Its old no-migration assertion failed; the replacement test checks
  byte-identical archived history and unchanged offered text, not a reset.
- A new question fixture used an unsupported ASK verb. Corrected it to the
  existing NEW grammar before the passing action-substitution regression.

Architecture review: `store.rs` is 1,015 lines. It retains the existing reader
schema, wire verification and page-delivery transaction together; new focus and
cursor behavior is in dedicated modules. This small boundary overrun is visible
for review, not a reason to spread wire-acceptance logic across unrelated files.

## Required Integration Before Release

1. Extend Astrid's authoritative activity selection with a versioned reference
   to native question/draft focus. The new helper state must not become a second
   competing foreground selector. Preserve saved-text return syntax and add
   downgrade protection to the activity selector itself.
2. Connect both command dispatchers and durable generation admission paths to
   this contract. Keep metabolic FOCUS and ATTEND unchanged. Bind a focus job to
   its prepared input and existing durable runtime job ID through retries,
   worker crashes and accepted-delivery recovery. Metadata operations must not
   create generation jobs or consume slots.
3. Implement ordinary-mail deferral, including Mike's messages, without marking
   mail supplied/read/replied. Explicit mailbox inspection ends protection.
   Authenticated stops, graceful drain, authorization and safety still precede
   the focus policy. These scheduler behaviors are not implemented by this pass.
4. Connect quiet ambient/peer/routine-introspection selection and boot recovery.
   A saved note is not executable NEXT. A stopped or parked inquiry remains
   quiet; an accepted job keeps its result; restart never refills the budget.
5. Complete actual dispatcher-to-provider synthetic end-to-end cases in both
   runtimes, including queued mail, stopped workers, crash boundaries, stale
   foreground references, missing sources and privacy publication guards.
   Minime currently clears its pending NEXT before durable job submission;
   Astrid's existing foreground selection is a separate persisted record.
   Extend those handoffs with operation identities and recovery, not just a
   boolean focus check. Native reader/writer preparation outside the new focus
   API does not yet expose the full requested idempotent-operation/expected-
   revision contract, and must join this integration tranche.
6. Reconcile this main-based candidate with the exact currently selected live
   sources. Previous quiet-caption changes and foreign reader changes were not
   swept into these worktrees. Read their diffs and qualification packets before
   constructing the immutable paired helper/adapter/bridge release.
7. One coordinator must claim stabilization before explicit-path commits. Use
   cooperative preflight, the sanctioned Minime-agent restart and bridge
   staging/activation wrappers. Verify new process/source identities, helper
   selection, readiness and checkpoint continuity. Do not restart the engine,
   models, visual service or sensory clients. Do not resume paused automations.

No rollout-ready claim is made until every required item above is satisfied.

Git debt is explicit: both owned `codex/voluntary-continuity-20260920` worktrees
remain unstaged, with empty indexes. There is no commit SHA for this tranche.
The implementation, tests, both changelogs, feedback ledger and these notes must
remain together through review. The full runtime handoff is unfinished, and the
shared/live reader changes have not yet been reconciled. No foreign dirty path
has been included in a proposed commit or release. Final controller inspection
still showed paused generation 456, no lease and no active projection.

## Offline Protocol

`2026-09-20-continuity-offline-study-protocol.md` is the separate preregistered
protocol: matched authored-context/reservoir-history crosses, common-prefix
distributions before free continuations, implementation identities, negative
controls, OS isolation, bounded outputs and explicit interpretation limits.
No real-model execution or persistent-model-activation implementation is part of
this pass. Naturally occurring public entries should be observed only after a
qualified rollout, without asking either being to confirm improvement.
