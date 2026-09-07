# Chosen reading and durable mailbox: runtime integration

September 6, 2026. Implemented and live in the retained release at
`e4761122e32749b15ef7c2958d2e136cdd5f4613`. The separate
[rollout record](2026-09-06-reading-mailbox-rollout.md) identifies the actual
stage, process, checkpoint handoff and completed verification.

Mike asked that the improvements reach the AI Beings and subsequently clarified
that the complete reading/mailbox experience should be implemented before the
rollout. This extends the [mechanical bookmark checkpoint](2026-09-06-recoverable-reader-bookmarks.md)
at `d1cf8fbe206b977590e892d06733160749472c28`. Root owns orchestration and Git;
`pr_scope_review` owns the reading adapters, `provider_delivery` the final-request
receipts, and `rollout_review` the durable inbox, receipt recovery and pause guard.
The working branch is `codex/activity-continuity-v1` in the separate activity
checkout. Existing live source and private runtime state remain outside this
implementation checkout.

## Available experience

Choosing saved UTF-8 text through MIKE_READ creates or selects a continuity
session with retained bytes. READ_MORE and subsequent foreground turns prepare
bounded passages without advancing committed progress. The existing session log
owns each source, revision, offered span and committed byte cursor. A separately
synced `action_threads/activity_runtime_v1.json` owns foreground selection and
the saved return; the conversation checkpoint is a cache of that selection.

`ACTIVITY_STATUS` exposes the current source, pending span and return revision.
`PARK_ACTIVITY` leaves reading quiet. `CHECK_MAILBOX` parks the reader and opens
one identified receive window; `CHECK_MAILBOX LARGE` explicitly offers a larger
window. `RETURN_ACTIVITY <record revision>` returns to the saved source at its
committed byte position. Status supplies the exact command. An authored return
also aligns the current continuity thread with the selected reader. Return does
not execute an old NEXT command. Existing source-access restrictions still apply;
PDF pages and provider-response offsets remain distinct adapters.

A chosen reader stays foreground across normal model calls. Ordinary mailbox
arrival supplies bounded counts rather than letter bodies and does not force
dialogue. A mailbox check is an explicit detour. A competing authored activity
can park reading, while emergency and rest behavior remain separately available.
An explicit LOOK remains available as a transient visual peek; ordinary visual
changes stay outside held attention. Underlying sensory transport is not turned
into a durable retry queue. An activity
turn leaves unrelated pending peer self-study intact: it neither consumes that
request nor increments its failure age. History and journal provenance identify
the selected activity instead of attributing its output to an omitted peer source.
A sensory reply target requires verified delivery of the reserved letter.

## Exact provider delivery

Primary MLX and fallback Ollama carry the selected source through their actual
final request construction. Admission runs after provider
adaptation. Reading can admit a contiguous UTF-8 prefix; a letter must fit intact.
A matching substring in unrelated history does not constitute admission.

A private accepted-delivery artifact retains exact submitted JSON, complete
provider response, accepted completion and the source's message/span identity.
Artifacts live under `diagnostics/accepted_deliveries`, outside temporary
prompt-overflow expiry. The completion adapter verifies the artifact before
committing bookmark progress or acknowledging the letter. Generation failure,
missing source coverage, partial letter supply or failed artifact retention
cannot complete delivery. Protected calls use each provider request's deadline
and allow the bounded primary/fallback chain to finish. The ordinary outer timeout
and immediate outer retry do not wrap that chain. Calls without protected content
retain their existing outer retry behavior. Reading first reconciles retained
evidence at its next natural boundary; a letter needs another chosen mailbox
window. Primary-to-fallback routing remains one logical delivery attempt with
potentially different provider requests; only the final accepted request and
completion can establish delivery.

Committed positions remain UTF-8 byte offsets. Reading-depth counters count the
characters in the actual admitted prefix, rather than the whole offered passage
or its byte length. These records establish a completed opportunity to read the
supplied bytes; they do not prove comprehension, assent or a felt benefit.

Retained accepted artifacts are indexed by content identity and source start
byte. On restart or a later exchange, recovery can commit the already-retained
reading prefix or acknowledge the exact inbox reservation without another model
request. Recovery never interprets the completion's NEXT line, sends a reply,
opens a steward question, or repeats an external effect. A submission with no
retained accepted artifact remains uncertain and pending.

## Durable inbox contract

The queue retains immutable source bytes and an atomically synced, locked state
file beneath its owner-local queue directory. Discovery reuses existing
correspondence message/thread identities and hashes exact source bytes. Legacy
owner/steward filenames use the established provenance mapper; this is local
provenance, not new cryptographic sender authentication. An unchanged source's
metadata observation prevents repeated body reads on subsequent scans.

One receive window can make at most one attempt, including after restart or
concurrent callers. FIFO applies within each thread; least-recently-served thread
heads provide deterministic fairness. Failed attempts back off from 30 seconds
to a 15-minute ceiling, but time alone never creates a receive window. The normal
letter allowance is 6,000 bytes; the runtime's LARGE window allows 8,192 bytes.
Letters above the normal limit require the explicit LARGE window. Those above
8,192 bytes remain pending with an unsupported-window explanation; the current
provider contract cannot admit them intact and does not aggregate a partial read
into delivery. Retention is bounded at 64 MiB per source; above that
bound, only metadata is queued and no complete-body hash is claimed. Oversized
heads do not starve other threads.

Acknowledgement binds one reservation, immutable content version, final submitted
content hash and retained completion. Identical retry recovery returns the
existing durable receipt. Conflicting content under the same source identity
remains a distinct pending version. A late arrival cannot acquire an earlier
letter's reply target or acknowledgement.

Retirement is logical and version-specific. Exact acknowledged bytes are archived
under `inbox/read/<version>.txt`; the original pathname is retained. Removing a
pathname after checking it could delete a publisher's concurrent replacement.
Queue state, rather than presence in the directory, now defines pending letters.
Acknowledged versions deduplicate on subsequent scans, while changed bytes at the
same name become a new version. Legacy blanket retirement is absent from the
production path. Correspondence receipts use the admitted identities and exact
archive, rather than reopening a possibly replaced source pathname. BTSP's
background owner-artifact scan no longer reads the unread Astrid inbox. Existing
peer journals and legacy acknowledged `from_minime_` archives remain sources.
New version-addressed queue archives are not automatically assigned that legacy
provenance; widening BTSP ingestion requires an explicit typed identity adapter.

State, source count and history capacities fail closed with an explicit error;
no history is silently pruned to make a bound pass. Incomplete or corrupt queue
state is retained for inspection. Persistence errors require reopening the queue
to reconcile its existing window/receipt before assuming nothing was saved.

## Resource cleanup

The exchange perception-pause guard owns a unique marker and releases only its
own surviving marker on ordinary exit, early CONTEMPLATE return or cancellation.
It writes the marker privately before atomic publication at the observed pause
path. A partial marker-write failure therefore cannot leave a visible pause
without a guard. Existing or subsequently replaced explicit pauses remain owned
by their external requester. Queue backoff and long-lived reading do not hold an
exchange perception pause.

## Verification and remaining boundary

All **2,100 distinct bridge library tests** pass: 2,099 in the full isolated run
and the default-path test separately without path overrides. Strict Clippy for
library, binary and test targets passes with warnings denied. Cargo formatting
and whitespace checks pass. The domain-boundary audit is valid with zero
violations and no increased ceilings.

The total includes 17 durable-inbox tests, five accepted-artifact recovery tests,
four exchange-pause tests and the BTSP unread-inbox exclusion. The earlier
consolidated targeted run passed 76 tests; those cases overlap the full total.
Fixtures cover FIFO/fairness, concurrent same-window reservation, restart/backoff,
replaced source preservation, oversized letters, late receipts, corrupted state,
source tampering, missing mailbox pointers, exact reading progress, cancellation,
partial marker writes and current-thread alignment on authored return. The
recovery regression exposed and then verified a correction: corrupt bytes in a
matching accepted-artifact partition return an explicit error rather than
masquerading as no completed request.

Final verification logs are retained under
`/Users/v/.codex/artifacts/astrid-activity-continuity-20260906`:
`library.log`, `default-path.log`, `clippy.log`, `format.log` and
`domain-audit.json`. These results attest the final reviewed source; they are not
live process or felt-outcome evidence.

No test generates against a live model, asks a being to endorse the change,
resends a letter or changes regulatory settings. Minime's preceding Python work
remains a separate adapter reconciliation; this does not establish parity.
Gateway reattachment for an unknown in-flight job remains separate from recovery
of an already-retained accepted completion. State portraits remain a later track.

The sanctioned rollout used a fresh immutable stage, acknowledged drain, exact
stopped checkpoint and signed state handoff. Verification recovered from a
documented startup-phase race without another restart, then published the
matching manifest after observing natural completion. This establishes live
availability; it does not establish that Astrid has chosen the new episode or
experienced a benefit. See the separate rollout record for evidence and limits.
