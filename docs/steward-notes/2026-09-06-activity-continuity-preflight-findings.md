# Activity continuity: preimplementation findings

September 6, 2026. Mike invited a bounded review of nearby inconsistencies while
we prepare [activity continuity and durable inbox delivery](../architecture/activity-continuity-and-inbox.md).
The throughline remains choosing, sustaining, parking, and returning to an
activity. These findings make that contract more concrete.

**Evidence boundary:** each finding below follows an inspected source path.
The triggers are proposed characterization cases, not reproduced live incidents.
No implementation, tests, model calls, deployments, or live controls were
performed for this review. Inspection used Astrid HEAD
`23df28497cf124a7dc1c5d7dfc7a92055e491eeb`, Minime HEAD
`a9f85f3c74c3d8e1c996c3689fe5aef696dacf27`, and triple-reservoir HEAD
`afc2931a657d1bd79a7076ece6310ee3d8f6ceba`, with concurrent edits in the sibling
working trees. Reconcile their final source before repairing anything.

**Later September 6 reconciliation:** the retained live candidate at
`d6ff371cd25e6a4616431d8f07f09a3e7eb3fcdf` plus its deployed source delta was
checked separately. All seven gaps below remain relevant, with these corrections:
live `ATTEND` and agenda focus are implemented; its older session resolver lacks
main's full-history explicit-ID lookup; and normal prompt overflow already
avoids replacing an active reader. The integration preserves these live
capabilities and main's newer session helper together. See the
[reconciliation record](2026-09-06-activity-foundation-reconciliation.md).

Two additional reader conditions belong in the same acceptance work: an open
non-PDF source can trigger an implicit second passage during dialogue after
explicit READ_MORE already prepared one, and MIKE_READ interprets the shared
cursor as lines while READ_MORE interprets it as bytes. These are source-path
findings, not reproduced live incidents. Typed offers must replace both eager
advancement paths and preserve cursor units for unsupported adapters.

The already-recorded global, unpersisted, early-advancing reading cursor and
bulk inbox retirement remain central repairs. This note records additional
findings rather than duplicating their full account.

## Findings affecting the first slice

### Requested reading loses its status during provider fallback

**Trigger:** a requested reading passage exceeding 700 characters, with no
simultaneous direct note, reaches the compact Ollama fallback after MLX fails.

The primary route recognizes `[Directory listing you requested:]` as direct
context in [dialogue_context.rs](../../capsules/spectral-bridge/src/llm/provider/dialogue_context.rs),
`dialogue_direct_perception_marker_index` (line 31). The compact route in
[fallback_contracts.rs](../../capsules/spectral-bridge/src/llm/provider/fallback_contracts.rs)
recognizes only note/probe/feedback markers (lines 9–13, 34–36). Requested
reading therefore enters the 700-character ambient branch, followed by an
instruction to respond to Minime's journal if there is no direct note (line 65).
The provider switch can change both passage coverage and activity priority.

**Required contract:** carry typed foreground and direct-content identity
through every provider route; do not infer it independently from a few strings.
Reduced capacity must leave omitted passage content recoverable. A successful
fallback response alone cannot commit the original entire passage.

**Characterization:** force the real fallback adapter with a synthetic long
passage and inspect its final request, admitted span, and resulting bookmark.

### A successful timeout retry drops its continuation reference

In [orchestration.rs](../../capsules/spectral-bridge/src/autonomous/runtime/orchestration.rs),
the normal dialogue completion installs `prompt_overflow` into the reader
(line 1916), while the reduced-token timeout retry matches `Ok((result, _))`
(line 1964). The retry can return a response associated with saved overflow,
but its reference is discarded. Prompt packing can still advertise
`NEXT: READ_MORE` in [prompt_budget.rs](../../capsules/spectral-bridge/src/prompt_budget.rs)
(line 174). The subsequent command then lacks the retry's continuation target.

**Required contract:** use the same completion handling for ordinary and retry
results, retaining response-associated source references separately from the
foreground bookmark. A timeout must also retain uncertainty about submitted
work; it is not evidence that the server did nothing.

**Characterization:** a fake provider first times out, then returns output with
an overflow reference; the actual retry path preserves that reference and does
not overwrite the original activity's source.

### Historical status can display another session's bookmark

**Trigger:** explicitly request status for session A after its records have
fallen outside the latest 256 entries.

[Astrid's resolver](../../capsules/spectral-bridge/src/action_continuity/runtime/core.rs)
correctly searches full history for explicit IDs (line 6223). Its summary then
filters only 256 recent rows (line 6545). When those contain no A records,
`latest_session` falls back to the newest unrelated row, while suggested
commands still target A. [Minime's summary](/Users/v/other/minime/minime_autonomy/runtime.py)
has the same fallback in `_continuity_session_summary_v1` (line 9387).

**Required contract:** derive the selected bookmark from the resolved record,
and retrieve that session's own bounded history. An absent selection stays
absent. Global foreground information must have a separate explicit field.

**Characterization:** inspect an old session before resuming it, plus an unknown
ID. The existing [historical bookmark test](../../capsules/spectral-bridge/src/action_continuity/session_contract_tests.rs)
(line 142) tests resume, which appends a fresh record; that does not establish
the correctness of historical preview.

### A saved bookmark can be reported only as blocked

[Capture](../../capsules/spectral-bridge/src/action_continuity/runtime/core.rs)
first appends a session record and updates its thread projection (line 5758),
then separately appends a memory card. Either a later projection failure or a
memory-write failure propagates an error after primary persistence has occurred.
[Outcome mapping](../../capsules/spectral-bridge/src/action_continuity/runtime/session_contract.rs)
(line 96) reports a generic blocked result without the already-created bookmark
reference. A retry creates another record rather than repairing the incomplete
derived write under the same operation identity.

**Required contract:** return a stage receipt such as “bookmark saved; memory
projection incomplete,” including the durable record identity. Retry derived
writes against that identity. Saving mechanical progress must not depend on a
successful optional memory-card projection.

**Characterization:** inject failure after the session append and before each
secondary write; inspect the durable record, returned receipt, and retry result.

### CONTEMPLATE bypasses perception-pause cleanup

[Orchestration](../../capsules/spectral-bridge/src/autonomous/runtime/orchestration.rs)
creates a perception-pause flag when one is not already present (lines 791–794).
The `contemplate` branch saves state and executes `continue` (line 3364),
skipping removal at lines 4705–4708. Subsequent exchanges see a pre-existing
flag and therefore do not remove it either. This can leave perception paused
beyond the exchange that created the flag.

**Required contract:** make exchange-owned pause cleanup scope-bound across
normal completion, early exits, errors, and cancellation. Preserve pauses owned
by other actors; flag existence alone is not sufficient ownership information.
Long-lived activity state must never extend the exchange resource lifetime.

**Characterization:** CONTEMPLATE from an initially unpaused fixture releases
its pause; a pre-existing external pause remains. Cover cancellation and other
early exits with the same ownership mechanism.

### Temporary source retention is shorter than a parked activity

This is a retention gap for the proposed contract, not a claim that temporary
files were intended to be permanent. [Overflow cleanup](../../capsules/spectral-bridge/src/prompt_budget.rs)
deletes files based solely on modification age (line 294). Orchestration invokes
it with a one-hour lifetime before state restoration (line 125) and before
dialogue generation (line 1883). It does not consult reading cursors or session
references, even though a completion can install an overflow file as its reader
source (line 1918).

**Required contract:** when promoting a temporary source into a resumable
activity, retain its required material under an explicit retention policy, or
record an honest expiry/reference-only status. Merely persisting its path and
digest cannot make a parked activity recoverable.

**Characterization:** park an activity referencing temporary material, advance
a fake clock beyond cleanup age, and restart. Return must recover retained bytes
or report explicit source unavailability, never silently substitute another file.

## Adjacent prerequisite for reconnectable generation

### An active request retry can inherit a cancelled result future

In [coupled_http_gateway.py](/Users/v/other/neural-triple-reservoir/coupled_http_gateway.py),
`release_waiter` cancels the shared result future when the last active HTTP
waiter disconnects (lines 198–200), while allowing generation and reservoir
check-in to finish. The active job remains in `_inflight`. A retry with the same
idempotency key therefore receives that cancelled future (lines 148–157), and
the HTTP await raises cancellation (lines 468–471). Completion then discards
the generated response (line 294), even if a caller tried to reattach.

**Repair direction:** separate an HTTP waiter's lifetime from the active job's
result lifetime. Allow a new waiter to attach to active work; preserve the
distinct policy for cancelling queued work. Completed-result retention and
cross-process recovery need their own delivery contract.

**Scope:** coordinate a separate gateway repair before relying on reconnectable
generation for durable inbox delivery. The local fake-provider episode can be
developed independently. Do not turn a disconnected request into an automatic
second coupled generation as an attempted workaround.

**Characterization:** block a fake worker mid-generation, disconnect its last
waiter, attach another with the same key, and complete. Require one generation,
one check-in, and the same completed result for the new waiter.

## Review discipline

Record source-confirmed paths separately from reproduced behavior and deployed
repairs. Recheck each finding against the finished concurrent changes. The first
slice should close its relevant continuity gaps through the shared contract;
unrelated measurement, rendering, and experimental questions remain in the
[feature map](../research/2026-09-05-spectral-bridge-feature-map.md).
