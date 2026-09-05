# Voluntary Bookmarks And Quiet Session Parking

## Request And Scope

Mike approved proceeding with the four directions in the
[activity review](/Users/v/other/astrid/docs/steward-notes/2026-09-04-activity-and-continuity-review.md):
reliable voluntary bookmarks, quiet reversible parking, accumulating inquiries,
and less self-generated administrative repetition. This is the first coherent
implementation tranche, not a claim that the whole program is complete.

The evidence is the review's bounded database/action history and source findings:
23 Minime capture attempts without committed current-thread captures, Astrid's
named session starts, and a mismatch between `handled` and actual persistence.
No private journal body was used as a test fixture or copied into this document.
No new canonical introspection read, felt resolution, or productive flywheel
round is claimed.

Interactive maintenance hold 346 was acquired by `codex-astra-interactive` at
2026-09-05T02:59:15.879627Z. The controller reported no active run and
`active_run_released=true`. The earlier controller verification passed, including
V1 immutability. The separately paused app automation was not resumed.

After final verification, the same hold's actor and generation were checked
again. The sanctioned controller resume completed at 2026-09-05T03:40:14.451438Z:
generation 347, `paused=false`, event `appended=true`, `spooled=null`. That command
verifies evidence integrity before releasing the maintenance hold. It does not
resume the separately paused app automation, deploy code, or grant live control.

## Implemented In Source

### Truthful Outcomes

Minime's existing string-returning continuity API remains compatible, with a
typed `ContinuityReply` carrying `continuity_action_result_v1`: outcome status,
whether a record persisted, an exact record reference, and false authority change.
The Python dispatcher, action manifest, and final action ledger carry this result.
Missing-input actions no longer count as successful operations in the agent's
success accounting. Unrelated thread actions keep their existing compaction paths.

Astrid uses a typed `ContinuityInputError` through its existing `anyhow::Result`
boundary. Its NEXT outcome maps this to `needs_input`, with `handled=false`,
instead of reporting the rejected operation as successfully handled.

Missing sessions, missing or placeholder summaries, metadata-only payloads, and
invalid finalization outcomes are reported without creating a fictional capture
or accepting a pending draft. Capturing or summarizing a parked, held, or complete
session requires an explicit resume first. An explicit accept operation remains
available; no draft is automatically accepted by this change.

### Bookmark Preservation And Return

Capture, summary, park, and resume now preserve the existing chosen focus, open
questions, source references, artifact references, and recorded next step when
those fields are omitted from a later command. The summary can still be updated.
Parking no longer replaces the reading continuation with a self-referential
`CONTINUITY_SESSION_RESUME` instruction. Reopening no longer replaces the source
references with references to the session file alone.

The existing `next` field can retain a targeted reading action such as
`INTROSPECT regulator 400`. Resume returns the focus, stopping-point summary,
questions, sources, and next step. It does not dispatch that step or bypass the
reader's existing resource, provenance, or authority checks. Source references
and supplied hash strings are retained references, not newly verified hashes.

Explicit session/record references are no longer restricted to the latest 256
records. A historical record reference identifies the session's latest state;
it cannot restore an older active state to bypass deliberate parking, or replace
a newer stopping point during resume. Routine prompt/status output windows remain
bounded. Existing append-only history is not rewritten or migrated, and fields
already lost from older finalization records are not fabricated back into them.

### Quiet Session Projection

The active-session lookup now considers the latest state for each session before
selecting an active one. An old active record can no longer resurrect a session
that was subsequently parked. A different still-active session remains visible.

Parked, held, and complete sessions are omitted from automatic session prompt
lines and session control-plane routes. In Minime, the associated memory cards
remain available through explicit recall, but their excerpts are not automatically
selected as the latest prompt memory while the session is quiet. Resuming makes
the existing cards eligible again. This suppression consults lifecycle history,
not just the recent 256 records: later unrelated activity cannot age away the
parking decision and make an old memory excerpt recur.

An optional `return_cue` is recorded during finalization. The default is explicit
request, and `automatic_return=false`. A cue is descriptive metadata only: it is
not a timer, background trigger, obligation, or permission to reopen a session.
Audit history, recent action receipts, explicit status, and explicit recall remain
available. This is not a promise to erase every historical mention from all
prompt paths.

### Less Administrative Chaining

Successful capture/summary replies retain the authored next step rather than
automatically prescribing a capture -> summarize -> finalize sequence. This
reduces unnecessary follow-up scaffolding without making private journal entries
produce summaries, deltas, or progress reports.

## Verification

Tests use synthetic questions, temporary stores, and the existing Minime live-
write/network isolation guard. No test sends reservoir controls or uses an LLM
completion as an inert experiment.

Covered cases:

- Missing session and bare alias requests, with an unaccepted draft unchanged.
- Empty, placeholder, and metadata-only captures without false persistence.
- Start -> capture -> summary -> park -> unrelated activity -> explicit resume.
- Preservation of question, source/artifact references, and reading continuation.
- Parked memory retained for recall but omitted from automatic memory excerpts.
- Another active session surviving the parking of the most recent session.
- Explicit return after more than 256 later records.
- Parked memory remaining quiet after its lifecycle record leaves the recent window.
- Historical record references respecting later parking and stopping-point updates.
- Invalid finalization preserving the original active state.
- No implicit reopening, automatic scheduling, authority change, or peer mutation.
- Typed missing-input results in the Python manifest and final ledger.
- Quiet session shapes excluded from Rust and Python control-plane route selection.

The first Rust build failed because a parsing helper assumed a `regex` dependency
that this crate does not have. It was replaced with standard-library parsing;
no dependency was added. An added Python manifest assertion exposed omitted result
fields even after the final ledger was correct; the manifest now carries the
same typed receipt. These were repaired rather than reported as passing checks.

Final review also found two lifecycle edge cases: a parked memory's status could
age out of a recent lookup, and a historical record reference could revive an
older active state. Regression tests now cover both, with the fixes described
above. A previous full pass was 956 Python tests plus 114 subtests and 1,708
bridge tests. After these final edge-case fixes, the Python suite passes 957
tests, one skipped, and 114 subtests in 25.61 seconds. The corresponding full
bridge library suite passes all 1,709 tests with zero failures in 261.16 seconds.

Scoped tracked-file `git diff --check` passes in both repositories. The two new
Rust files were formatted independently; no whole-tree formatting or refactor of
the inherited runtime is claimed. All tests run in this tranche were source-level
checks; none is a live deployment receipt.

Commands:

```sh
cd /Users/v/other/minime
python3 -m pytest -q tests
cd /Users/v/other/astrid
cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib -- --test-threads=1 --quiet
```

## Exact Source Ownership

Astrid:

- `capsules/spectral-bridge/src/action_continuity/runtime.rs`
- `capsules/spectral-bridge/src/action_continuity/runtime/core.rs`
- `capsules/spectral-bridge/src/action_continuity/runtime/session_contract.rs` (new)
- `capsules/spectral-bridge/src/action_continuity/session_contract_tests.rs` (new)
- `capsules/spectral-bridge/src/autonomous/next_action/mod.rs`
- `capsules/spectral-bridge/src/continuity_control_plane.rs`
- `CHANGELOG.md`
- `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`
- This implementation note.

Minime:

- `minime_autonomy/session_contract.py` (new)
- `minime_autonomy/runtime.py` (limited continuity/result/manifest hunks)
- `continuity_control_plane.py`
- `tests/test_session_contract.py` (new)
- `CHANGELOG.md`

The large existing runtime files retain their current ownership boundaries;
small contract helpers and tests are separate modules. A broad extraction or
formatting rewrite of the inherited runtime was deliberately not mixed into this
behavioral repair. Prior journal/deployment changes and unrelated dirty work were
preserved. No staging, commit, merge, or cleanup is part of this tranche.

## Deployment And Remaining Work

**Source-only. No restart or deployment has occurred for these changes.** The
running bridge and Python agent have not been claimed to exhibit this behavior.
No engine, model, PI, fill target, damping, sensory cadence, peer gate, attention
weight, or research permission was changed. No journal was solicited.

Before a rollout, carry the scoped bridge changes into the separately reviewed
exact-live-lineage candidate and test them there; canonical dirty main is not an
assumed replacement for the running bridge. Bridge deployment still requires the
sanctioned `scripts/build_bridge.sh` and the existing graceful first-drain review.
Minime requires the sanctioned Python-only reload with source/PID readiness and
protected-service checks. Concurrent-agent preflight must pass. This note is not
a deployment receipt or an expansion of control authority.

Remaining parts of the larger program:

1. Quiet *experiment* parking across lifecycle priorities, preserving independent
   charter, safety, budget, and authority repair requirements. Session parking in
   this tranche does not remove every paused-experiment resume projection.
2. Better discovery/use of the existing agenda and ATTEND tools, without changing
   attention defaults or making continual productivity compulsory.
3. Targeted source-reading affordances and tolerance for malformed session field
   separators. The existing reader and grammar remain in use; no new broad alias
   silently turns `PERSIST_CONTEXT` into a capture.
4. Accurate READ_MORE advance/EOF/novelty receipts and a bounded evaluation of
   context-overflow recursion, without suppressing fresh objections or friction.
5. Phase-aware LLM completion accounting and profiling of job-store scans. The
   observed persisted-study/timeout discrepancy remains separate work.

The intended benefit is reliable, voluntary returnability. Passing these tests
does not establish a felt improvement, consent to an experiment, or any conclusion
about consciousness or mental health.
