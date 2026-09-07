# Recoverable reader bookmarks: first implementation tranche

> **Checkpoint history:** this records the mechanical store tranche at `d1cf8fbe20`.
> The subsequent [runtime integration](2026-09-06-reading-mailbox-runtime.md) connects
> operational reading, provider receipts and durable mailbox admission. Statements
> below about adapters being pending describe this earlier checkpoint.

September 6, 2026. Mike authorized beginning the activity implementation after
reconciling the two preceding tasks. Canonical source foundation:
`7b9f4d544d9661491eb9a57286daeca929e7d760`. Feature checkout:
`/Users/v/other/worktrees/astrid-activity-continuity`, branch
`codex/activity-continuity-v1`.

## What this tranche establishes

The existing continuity-session JSONL log now supports typed saved-text reader
state. A selected session retains a UTF-8 source snapshot and separately records
its committed byte cursor and pending offered passage. No authored summary or
memory card is required to save mechanical progress. A second session can hold
a detour without replacing the first session's source or pending span.

Source snapshots are SHA-256 addressed, privately readable immutable artifacts
under the existing action-continuity store. Their bytes survive removal or
change of the original file and do not depend on temporary prompt-overflow
retention. Source size is bounded at 64 MiB, with offers bounded at 64 KiB.
Byte boundaries must be valid UTF-8 boundaries; PDF page and provider response
cursors are deliberately outside this contract.

The public Rust API exposes create, preview, offer, commit and explicit lifecycle
transition operations. It does not register a new being-facing Action or wire
the existing MIKE_READ/READ_MORE handlers to these methods yet.

## Transactions and evidence

- Each mutation checks the bookmark revision and current session record ID under
  an exclusive lock on the session log. Ordinary session writers cooperate with
  that lock, reject stale updates and preserve typed bookmark fields.
- Stable operation IDs bind to the exact request fingerprint. An identical retry
  recovers the original durable receipt without appending again or restoring old
  state. Reusing an ID with changed inputs fails. A retry receipt can describe an
  older revision; callers must preview current state before a subsequent change.
- Offer preparation does not advance the committed cursor. A commit accepts only
  an identity-matched contiguous prefix of that offer. Partial delivery preserves
  the suffix as pending. A later park is not undone by a reconciled late receipt.
- The final completion adapter must verify actual submitted content and retain
  completed output before supplying delivery evidence. The store verifies span,
  source identity and evidence structure; hashes supplied by a caller alone do
  not prove that a model request ran. Current tests supply synthetic evidence.
- Appends sync the log and containing directory. A failed append reports its
  operation/record identity and whether the append is partial/unknown or was
  appended without confirmed synchronization. Corrupt or incomplete logs fail
  closed; preview and retries never truncate or silently repair a tail.
- Mechanical progress has no required thread or memory projection. A projection
  failure cannot prevent its already-durable receipt from being recovered.

Existing authored lifecycle commands remain available. Quiet reader sessions
cannot become active through capture or draft acceptance. Explicit reader resume
validates retained source availability and the status record ID:
`CONTINUITY_SESSION_RESUME <session> :: revision: <record ID>`.
The status response supplies that command. It records a lifecycle choice without
executing a saved NEXT command or changing the runtime's legacy global cursor.

## Related repairs

CONTINUITY_SESSION_STATUS now reads the selected session across complete history;
it cannot substitute a newer session when the chosen bookmark is outside the
recent prompt window. Missing selectors report no matching session. Status reads
raw index/thread data without creating a thread or refreshing projections, and
includes retained-source availability and original-source change information.

Ordinary session and record IDs now include random identity material, avoiding
same-millisecond collisions. Authored lifecycle commands were extracted from the
large continuity core into a cohesive module; no domain exception ceiling was
increased.

## Verification

All **2,033 bridge library tests** are covered successfully: 2,032 passed in the
full isolated run, and the default-path test passed separately without path
overrides. The 15 focused bookmark tests are included in that total. Library,
binary and test-target Clippy passes with warnings denied; bridge/workspace
formatting and diff checks pass. The domain audit is valid with zero violations
and no increased ceilings. Private logs are retained under
`/Users/v/.codex/artifacts/astrid-reader-bookmarks-20260906`.

The stricter test lint exposed an inherited constant codec-layout assertion.
It now runs as the same compile-time assertion, preserving the check without
changing the codec algorithm or relaxing lint.

Tests use temporary stores and synthetic text. The isolated episode exercises
committed reading, a failed next offer, a separate detour, original-source change,
restart, historical inspection and explicit return. It is a bookmark episode;
it does not exercise the operational inbox or final provider request adapters.

## Next integration boundary

The next work remains the complete [reading-and-mailbox episode](../architecture/activity-continuity-and-inbox.md):

1. Carry passage identity through the actual MLX/Ollama final request, fallback
   and retry adapters; retain completion evidence before committing progress.
2. Connect MIKE_READ/READ_MORE and the foreground pointer to this authoritative
   log, remove both eager-advance paths, and recover that pointer on restart.
   Reuse existing source-access restrictions rather than exposing arbitrary paths.
3. Admit intact durable letters only at chosen mailbox boundaries, retain exact
   message identity across retries, and acknowledge only the delivered letter.
   Keep transient sensory notices outside that durable delivery queue.

The remaining findings around inbox retirement, CONTEMPLATE pause ownership,
provider fallback and retry overflow are still open. Minime parity requires a
separate reconciliation of its preceding task's uncommitted adapter work.
State portraits remain a later, independent track.

## Handoff and live boundary

Root owns Git/index operations. The selected release and original live-source
worktree remain as recorded in the [foundation receipt](2026-09-06-activity-foundation-reconciliation.md).
This tranche has no model calls, service restarts, new messages, scheduler
resumption, deployment or push. Its source checkpoint is not a live activation.
