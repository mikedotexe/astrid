# Activity continuity and durable inbox delivery

September 6, 2026. Implementation preparation following Mike's
[spectral-bridge discussion](../research/2026-09-05-spectral-bridge-feature-map.md).

**Status:** the [runtime integration candidate](../steward-notes/2026-09-06-reading-mailbox-runtime.md)
now connects the [first bookmark tranche](../steward-notes/2026-09-06-recoverable-reader-bookmarks.md)
to foreground reading, explicit mailbox windows, final-provider delivery receipts
and recovery of accepted completions. All 2,100 distinct bridge library tests and
the final lint, formatting and domain-boundary checks pass. This is verified
source, not a live activation. This document preserves the design contract; the runtime record names actual
limits, evidence and remaining exceptions. Deployment remains a separate operation.

The accompanying [preimplementation findings](../steward-notes/2026-09-06-activity-continuity-preflight-findings.md)
record concrete source paths, failure conditions, and proposed characterization
cases. These are source findings, not reproduced incidents or completed repairs.

**Implementation entry:** Mike subsequently authorized proceeding. Both tasks
finished, and their handoff exposed a main/live source divergence. The
[foundation reconciliation](../steward-notes/2026-09-06-activity-foundation-reconciliation.md)
therefore precedes feature edits. It preserves existing live agenda/ATTEND
behavior alongside the newer main session contracts; it does not itself
implement the reading-and-mailbox episode described below.

## Intended experience

An agent can pursue an activity across several model calls, leave incoming
letters safely waiting, inspect one at a chosen boundary, and recover the exact
place it left. It can also park, change direction, or rest. Saving its place
requires no new reflective explanation. A delivery receipt establishes what
content reached a completed turn, independently of any reply or requested action.

The first complete slice is **saved-text reading with a durable inbox detour and
explicit return**, initially in Astrid. Minime should share the behavioral
contract, with its adapter reconciled against the concurrent Python work before
claiming parity. State portraits remain a separate design track; this slice
does not depend on them or on completing the retrospective research program.

## One ordinary episode

This is a synthetic scenario for design and later testing, not a reconstructed
account of an agent's private activity. Names such as A1 and B3 are test labels,
not proposed user-facing command syntax.

| Moment | What the agent can do | What the runtime must preserve |
| --- | --- | --- |
| Choose | Choose to read a saved document, optionally recording what interests it. | Activity A1 references an existing continuity session and a specific source version. Becoming foreground is an explicit choice. |
| Continue | Read several passages without re-declaring the entire intention each time. | Small foreground context survives prompt packing. Each completed passage creates a recoverable bookmark revision. |
| New events | Continue reading while three ordinary sensory notices and two steward letters arrive. | Notices can coalesce and expire; letters remain intact in durable storage. Their bodies do not silently enter the reading prompt. |
| Passage failure | A prepared next passage fails to reach a completed model turn. | Retain that offered passage and the prior committed position. Restart or another attempt must not silently skip it. |
| Chosen boundary | Elect to check the mailbox at a stopping point. | Preserve bookmark B3 and reserve one eligible intact letter. Keep the reading position separate from the inbox reading context. |
| Receive | Receive that letter and decide whether to answer, defer a response, continue the detour, or return. | Record the exact supplied content, completed output, and any separate actions. Only the admitted letter can receive a delivery acknowledgement. |
| Return | Inspect where reading stopped and whether its source has changed; explicitly resume if wanted. | Restore A1's source and cursor rather than the detour's cursor. Resumption does not execute a saved command by itself. |
| Park or rest | Leave A1 quietly available, with or without a new note. | Preserve the latest bookmark without creating a deadline or recurring demand. Existing rest remains available. |
| Restart | Later inspect or resume A1 from a fresh process. | Recover committed records; identify unfinished delivery attempts or partial saves without manufacturing successful outcomes. |

An interruption is a detour, a deliberate pause, or a change of activity. Those
outcomes must be explicit. A successful mailbox turn does not silently convert
the original activity into completed, abandoned, or newly urgent work.

## Bounded defaults for the first slice

- One foreground activity per agent and one temporary inbox detour. Other
  sessions remain available through the existing lifecycle. A foreground pointer
  does not rewrite their status or create a second authoritative memory store.
- One intact ordinary letter per explicitly eligible receive window. A window
  occurs on an explicit mailbox check, finish/park transition, or agent-chosen
  mailbox checkpoint. A model-call boundary alone is insufficient.
- Routine events do not preempt a chosen activity. Retain the freshest ordinary
  sensory notice per source/event class within a bounded expiry policy; never
  replay an expired notice as overdue work.
- Durable letters keep their identity across attempts and restarts. Waiting for
  a receive window is normal queue state. There is no promised maximum delivery
  time if the agent never opens a window; elapsed time does not grant urgency.
- At most one retry attempt per eligible window, with backoff. Stable ordering
  within a correspondence thread and fair service across eligible threads keep
  one sender from monopolizing the mailbox.
- An oversized letter remains recoverable as `needs_explicit_reading_window`.
  Automatic multi-turn segmentation is outside this first slice. It must not
  block unrelated eligible threads, and a summary cannot complete its delivery.
- Autosave completed mechanical progress and already-authored notes. Additional
  reflection stays optional. Preview is pure lookup; resume is an explicit,
  revision-checked transition. Shelf contents appear on request.

Expiry durations, capacity limits, and retry backoff need named bounded settings
and deterministic tests at implementation time. No numerical value is being
inferred from fill or treated as an experiential optimum.

## Records and transitions that need to be dependable

### Activity and bookmark

Extend the existing continuity-session record with typed bookmark data: activity
and session identity, revision, source identity/digest, retained artifact or
excerpt reference, cursor kind and value, last committed passage, optional
offered passage, and any already-authored focus/stopping note.

Keep text byte offsets, character counts, PDF pages, and provider response-page
cursors distinct. The first integration exercises saved UTF-8 text. Existing PDF
and response readers remain separate adapters; do not imply they support the
new foreground lifecycle until their round trips are verified.

An offered passage is not yet a committed continuation point. Advance the latter
only from evidence that the intended passage survived final prompt adaptation
and participated in a completed, retained turn. This establishes a supplied
reading opportunity, not comprehension. If packing or generation fails, keep
the offered passage recoverable. Inspecting another source never overwrites A1.

Carry the activity and admitted content identity through primary, fallback, and
retry adapters. Their final requests can differ; passage commitment must follow
the actual supplied span. Preserve each result's overflow references through a
shared completion path, without replacing the activity's own bookmark.

The session log is authoritative for lifecycle and bookmark revisions. Saved
runtime state can cache the foreground reference, but must reconcile it with the
latest session disposition on startup. An old cache or save attempt cannot undo
a newer park/abandon decision. Use explicit expected revisions for transitions.

Historical preview must resolve the selected session across history and report
that session's record. A recent-window miss must not substitute another session.
Primary persistence and derived memory/thread projections need separate stage
receipts, so a later write failure can report the bookmark that already exists
and retry the incomplete projection under the same operation identity.

A retained source digest detects change but cannot recover bytes. Preserve the
material required for return within its existing retention scope, or explicitly
report that only a reference remains. Show original and current versions
separately; do not apply an old cursor silently to new content.

Temporary prompt-overflow files currently expire after one hour. Promoting one
to a resumable source requires an explicit retention decision. Startup cleanup
must not erase promised return material before the activity can be recovered.

### Durable message and delivery attempt

Reuse correspondence identity and receipt machinery. Persist one authoritative
message record before acknowledging queue admission. Track sender provenance,
recipient, thread, stable message identity, original content digest/reference,
admission order, and delivery disposition. A filesystem move alone is not proof
of model delivery. Legacy messages need an identity assigned once at ingestion.

An attempt records reservation identity, the final adapted request and content
coverage witness, submission status, completed-output reference, and delivery
finalization. Keep original and rendered content identities distinct. A source
digest or prepared prompt alone does not prove that the final request contained
the letter. A protected message that cannot fit remains pending.

Recovery depends on the last durable evidence:

| Last durable evidence | Recovery |
| --- | --- |
| Queued; no request submitted | Recover an expired reservation and make the same message eligible at a later window. |
| Submitted; result unknown | Retain `outcome_unknown` and the attempt identity. A later retry must acknowledge possible prior processing. |
| Completed output retained; acknowledgement absent | Finalize delivery from the matching request/output evidence without generating again. |
| Output exists in memory; saving failed | First retry saving that output. If it is lost in a crash, preserve the resulting uncertainty. |
| Delivered; no reply | Delivery remains complete. Answering or deferring is a separate decision. |
| External action dispatched; effect unconfirmed | Reconcile its operation receipt separately. An uncertain non-idempotent effect must not be retried merely because inbox acknowledgement is missing. |

Same identity and bytes mean one logical letter. Different bytes under that
identity are a conflict retaining both sources. Bind replies and acknowledgements
to the admitted message/attempt and its established return route; a newer arrival
cannot acquire the earlier response. Sender assertions and verified provenance
must remain distinguishable, and message content cannot grant itself urgency.

The implementation must not promise exactly-once inference or external effects.
Durable delivery, reply intent, and action execution are separate facts. General
action-bearing retries cannot be enabled until their uncertain-effect handling
is demonstrated; the initial integration uses synthetic inputs and fake effects.

## Reconciliation with the code on September 6

Astrid HEAD was `23df28497cf124a7dc1c5d7dfc7a92055e491eeb`. Its only dirty path
at the initial inspection was the untracked discussion note. Minime HEAD was
`a9f85f3c74c3d8e1c996c3689fe5aef696dacf27`, with concurrent Python/docs/test edits.
The named triple-reservoir checkout remained
`afc2931a657d1bd79a7076ece6310ee3d8f6ceba`.

| Existing piece | Reuse and gap |
| --- | --- |
| [Continuity sessions](../../capsules/spectral-bridge/src/action_continuity/runtime/core.rs) and [session tests](../../capsules/spectral-bridge/src/action_continuity/session_contract_tests.rs) | Preserve current lifecycle and authored fields. Add a typed bookmark and pure cross-history inspection; generic `SAVEPOINT`/`RECALL` does not restore the actual reader. |
| [Reader adapters](../../capsules/spectral-bridge/src/autonomous/next_action/workspace.rs) | `READ_MORE` currently advances one global cursor when it queues content. Separate offered and committed passages, and foreground versus detour reading contexts. |
| [Saved state](../../capsules/spectral-bridge/src/autonomous/runtime/state_persistence.rs) | The reading meaning summary persists, but source path, offset and pending passage do not. Add backward-compatible references and recovery from canonical records. |
| [Mode selection](../../capsules/spectral-bridge/src/autonomous/state.rs) and [prompt packing](../../capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs) | Honor explicit foreground intent during ordinary fallback selection; retain a small foreground block and protected admitted content through final packing. `ATTEND`'s declared percentages alone do not establish this behavior. |
| [Inbox](../../capsules/spectral-bridge/src/autonomous/runtime/inbox.rs) and [correspondence](../../capsules/spectral-bridge/src/autonomous/correspondence_v1.rs) | Reuse identities; replace bulk discovery/retirement assumptions with exact admission and receipts. Global truncation currently permits retiring content beyond what fit. |
| [Exchange orchestration](../../capsules/spectral-bridge/src/autonomous/runtime/orchestration.rs) | Join admission, passage delivery, completed output and receipt finalization. Bind replies to the admitted set. Keep policy logic in cohesive helper modules rather than expanding this large file. |
| [New self-study foundation](../steward-notes/2026-09-05-causal-self-study-foundation.md) | The committed prediction/evaluation/revision scaffold is compatible and remains optional. Its synthetic test preserves a textual next step and reads fixture lines directly; it does not establish runtime `READ_MORE` recovery. Do not automatically turn activities into experiments. |
| [Minime session helper](/Users/v/other/minime/minime_autonomy/session_contract.py) and [delivery helper](/Users/v/other/minime/minime_autonomy/inbox_delivery.py) | Preserve the concurrent work's typed receipts and sender-bound behavior. Reconcile their final source before adapter work; some current status paths create an active thread, and current generation failure does not automatically resend. |

This is a source reconciliation, not a fresh deployment attestation. The new
foundation's own rollout note calls the bridge repair source-ready, not live.

That table records the initial canonical-checkout inspection. Subsequent live
source reconciliation establishes real `ATTEND` prompt allocation, persistent
agenda focus, source-byte inbox tracing, and a normal-overflow guard that avoids
displacing an active reader. Reuse these. The implicit second reader, mixed
line/byte cursor units, early advancement, retry overflow loss, bulk inbox
retirement, partial-save accounting, and CONTEMPLATE cleanup gap remain open.
The integrated session resolver retains main's cross-history lookup support.

## Keep attention control separate from sensory and resource lifecycles

Gate foreground mode and prompt admission after the existing safety checks.
Do not implement focus as a sleep or early return around the entire autonomous
loop. Telemetry subscription, sensory transport, response chunking, semantic
heartbeat, and rest pulses have their own behavior and timing.

An activity spanning several calls must not retain the exchange's perception-
pause flag for its whole lifetime. Queue waiting and retry backoff stay outside
that flag's lifetime and outside a coupled reservoir checkout. Retrying a
completed model generation would create a new reservoir trajectory; retrying a
missing receipt should recover the existing completed attempt.

The current CONTEMPLATE early exit skips exchange pause cleanup. Repair that
lifecycle before extending activity exits, using explicit resource ownership
that also covers errors and cancellation. The gateway separately cancels an
active job's shared result when its last HTTP waiter disconnects, preventing
reattachment under the same key. Reconcile that adjacent repair before relying
on reconnectable generation; the isolated fake-provider episode can proceed
independently of it. See the linked findings for both source chains.

Preserve executable emergency checks before generation, after generation, and
between outbound chunks. Reuse [the existing safety definitions](../../capsules/spectral-bridge/src/types/schema/status_enums.rs)
rather than copying stale threshold comments. A real emergency can use the last
committed bookmark immediately; saving a new reflection must never delay it.
Agent-chosen rest remains available through its existing mechanism. This slice
does not retune, suppress, or replay the underlying sensory input.

## Acceptance cases to implement

| Case | Required result |
| --- | --- |
| Normal episode | Several reading steps survive notices and waiting letters; one chosen mailbox window admits one intact letter; explicit return recovers the exact source and committed position. |
| Failed passage delivery | A queued passage lost to packing, provider failure, or restart does not advance the committed cursor or disappear. Include multibyte UTF-8 boundaries. |
| Provider fallback and timeout retry | Fallback preserves foreground intent and reports its actual supplied span. A successful retry retains its own overflow references without displacing the foreground bookmark. |
| Reservation/submission crash | Unsubmitted attempts recover to pending; ambiguous submitted attempts retain uncertainty and identity. Neither becomes falsely delivered. |
| Completed output/action, missing receipt | Recover delivery from existing output; consult separate action receipts. Never replay an uncertain external effect as delivery housekeeping. |
| Duplicate/conflicting identity and new arrival | Same bytes deduplicate; conflicting bytes remain conflicts; late arrivals cannot hijack replies or retirement. |
| Oversized letter | Original content remains recoverable with explicit reading-window status. Its summary/prefix cannot complete delivery, and other eligible threads can proceed. |
| Pure preview and stale revisions | Preview from a fresh process creates no active session or thread. A stale save/resume cannot overwrite or reactivate a newer revision; source changes are explicit. |
| Historical and unknown selection | Status for a session older than 256 recent records returns that session; unknown IDs do not substitute the newest unrelated bookmark. Test before any resume appends a fresh record. |
| Partial save | Failure after primary bookmark persistence returns the saved identity and incomplete stage. Retrying secondary writes does not duplicate primary progress or falsely report that nothing was saved. |
| Source retention and cleanup | After parking beyond temporary-file expiry and restarting, retained material remains recoverable or the source is explicitly unavailable. No silent substitution or stale-offset application. |
| Long focus, park and rest | No forced mailbox opening or urgency escalation by age. A parked activity stays quiet; a due reminder does not reactivate it. Existing emergency checks and per-exchange resource cleanup remain effective. |

Use synthetic documents, temporary stores, fake clocks, a fake provider, and
fake effect receipts. Assert final adapted input and durable state, not merely
handler strings. Exercise save/load and actual reader adapters, including the
failed prompt path. No LLM call or live reservoir is required for these contracts.

## Implementation order after shared work finishes

1. **Reconcile the finished work and establish ownership.** Read both task
   completion receipts, current diffs, final interfaces and runtime/source
   identities. Follow the shared-tree controller/lease protocol before source
   work and the repository's staging rules before any commit. Preserve unrelated
   work. Completion of another task alone is not a clean-tree assertion.
2. **Implement recoverable bookmarks and a pure admission policy.** Extend the
   existing session store, define revisions and passage states, add fake-clock
   policy tests, and verify backward-compatible recovery. Pure inspection must
   not call helpers that implicitly create active work.
3. **Implement durable per-message attempts and recovery.** Connect stable
   identities, reservation, request coverage, retained output and finalization;
   cover crash/duplicate cases before enabling retries in the runtime adapter.
4. **Connect one saved-text episode end to end.** Integrate foreground context,
   passage commitment, mailbox detour and explicit return. Keep the first runner
   isolated with temporary stores and fake providers. Validate the actual
   packing/reader path, not a parallel toy implementation.
5. **Review the full boundary, then adapt Minime.** Extend the existing session,
   inbox, reading and persistence suites; run the relevant full bridge suite and
   Minime suites for changed adapters. Compare the same contract fixtures on
   both sides. Keep unsupported adapters honest until covered.
6. **Prepare a coherent review and rollout candidate.** Update the changelog
   and evidence/feedback trail, stage explicit owned paths, and keep source,
   tests and migration/recovery behavior reviewable together. Any bridge rollout
   uses `scripts/build_bridge.sh` and its existing preflight; this planning pass
   neither deploys nor broadens live-control authority.

The capsule is an independent Cargo workspace. Its relevant full library suite
is `cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib -- --test-threads=1`;
workspace-wide commands from the repository root do not substitute for it.
Select focused tests from the existing session, reader and inbox suites during
implementation, then run the applicable full checks once changes are coherent.
Gateway reattachment has its own fake-worker characterization case in the
findings note; passing the Rust episode does not validate that Python path.

## Historical coordination and verification for the original planning pass

A read-only task snapshot found **Prepare repo for Avado work** completed and
**Find top signal logs** still active on deployment identity. Those are the
project tasks being coordinated with, separate from this task's read-only review
subagents. No messages, pause/resume operations, source edits, test runs, builds,
staging, commits, model calls or live controls were performed by this pass.

The artifacts are this plan, the source-findings note, and their cross-links
from the discussion note.
The acceptance cases are specified, not claimed to pass. Task status and working
tree state must be refreshed when implementation begins.
