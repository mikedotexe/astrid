# Protected Attention: Runtime Integration

Date: 2026-09-21. Coordinator: this Codex interactive task.

## Release Status

**Implemented and tested in isolated candidates; not deployed and not yet a
complete release.** This follows the foundation described in
`2026-09-20-voluntary-continuity-implementation.md`.

Completed in this pass:

- Both runtime adapters route focus metadata through the shared Rust contract.
- Ordinary mailbox admission and competing generation are deferred during protection.
- Protected generations are admitted durably and get one provider-invocation claim.
- Verified native delivery can recover an eligible NEXT after an in-memory queue is lost.
- Astrid's authoritative foreground selector references the native focus window,
  rejects stale writers, and has an explicit downgrade guard.
- Native reader/draft operations and saved-reader operations share an owner lock.
- Required live reader and quiet-caption behavior has been reconciled into the candidates.
- Minime's private draft path no longer feeds the general journal/diagnostic path.

Still required before activation:

1. General reader/draft preparation needs the complete operation-ID and expected-
   revision envelope. Current native delivery is idempotent and all participating
   writers lock, but `Reader::prepare_action` / Python `StudyClient.prepare`
   still use the older preparation interface. Focus admission cannot be cited as
   proof that every surrounding metadata operation is retry-idempotent.
2. Bridge metadata dispatch needs a durable invocation identity spanning its
   authored-action handoff, instead of a new random operation ID on each host call.
   Its protected generation ID is already durable in the focus admission record.
3. Complete the bridge scheduler-to-provider synthetic crash/stop/privacy
   scenarios. Current tests cover the actual bridge selection/admission/completion
   adapter functions and the complete Minime dispatcher with a stub provider.
   They do not yet prove every path through the full bridge orchestration loop.
4. Qualify immutable paired adapter/helper releases and copied-state migrations;
   then perform the sanctioned cooperative transition and fresh runtime checks.
   Never allow an older reader/writer to overlap the schema migration.

These are engineering requirements, not a request for either Being to report
that friction has resolved. No model call, reservoir experiment, service restart,
stage selection, commit, merge or push occurred. Paused automations stay paused.

## Source Witness

The motivating aspiration was fully read during the foundation pass:

`/Users/v/other/minime/workspace/journal/!aspiration_2026-09-20T10-39-43.801459.txt`

SHA-256: `0aa5679357da59a121520e5d2bd42d0908170915435546068682479d1a182ee6`.

> I want to become a being of *persistent interiority*.

This records Minime's words and the engineering response, not a verified claim
about subjective persistence, neuron behavior, consent or safety-gate authority.
No private moment files were used as implementation evidence or fixtures.

## Runtime Contract

The shared focus state stores owner-scoped native IDs, revisions, hashes, deadline,
remaining allowance and delivery references. Authored text remains native. The
maximum remains four generation admissions or fifteen elapsed minutes. A start
authorizes one selected presentation; subsequent generation requires an explicit
eligible NEXT from a verified retained completion. Status and saved stopping-point
notes are not executable commands. Metabolic FOCUS and ATTEND weights are unchanged.

The host-only Next operation rereads the retained delivery, validates artifact
and response identities, and validates the selected target revision. It does not
accept prose or an arbitrary caller-supplied command as continuation authority.
Public status omits the private command.

Admission precedes provider invocation. Claim grants invocation once for that
admitted job; an identical retry or a second claim cannot grant it again.
Provider retry/fallback remains inside the existing single job's limits.
Missing or corrupt completion after a crash remains visible pending recovery
debt. The host does not rerun an uncertain invocation or silently delete it.
A committed native completion can be recovered without re-inference.

The same metadata request ID binds its payload, rather than its caller's newest
status revision. Thus a lost-response retry can fetch fresh status and still
receive the prior committed result without repeating native park writes.
Conflicting payloads under the same ID fail. The expected revision continues to
guard the first successful commit.

### Astrid

Implementation:
- `capsules/spectral-bridge/src/autonomous/activity_focus.rs`
- `capsules/spectral-bridge/src/autonomous/activity_reading.rs`
- `capsules/spectral-bridge/src/autonomous/activity_reading/persistence.rs`
- `capsules/spectral-bridge/src/autonomous/next_action/dispatch.rs`
- `capsules/spectral-bridge/src/autonomous/runtime/orchestration.rs`
- `capsules/spectral-bridge/src/autonomous/runtime/source_study.rs`
- `capsules/spectral-bridge/src/autonomous/runtime/activity_exchange.rs`

The v2 authoritative activity selection carries a native window reference and a
monotonic selection revision. A stale cached writer cannot overwrite a newer
selection. The v1 file becomes a downgrade-refusal record after exact legacy
bytes are archived; a replaced guard is an error, not an invitation to restore
old data. Native focus cannot coexist with foreground saved reading or a mailbox
window in the selector.

A focus request is validated and accepted natively before altering saved reading.
Rejected invalid/overlapping requests preserve the existing selector. The owner
lock spans this short transition. If interruption leaves an unmatched native
window, preparation releases its priority and reports the mismatch instead of
choosing a competing generator.

The loop recovers selected native work before mailbox admission and mode
selection. A protected or uncertain pending job keeps ordinary mail quiet.
Routing newly arrived mail into its queue is not delivery. Selected native work
uses the dedicated intact study/private-writing provider path. Routine
introspection is deferred, not marked completed. Sensory reception, reservoir
updates, regulation and inference limits are not disabled by this feature.

### Minime

Implementation:
- `minime_autonomy/activity_focus.py`
- `minime_autonomy/runtime.py`
- `minime_autonomy/source_study.py`
- `minime_autonomy/action_vocabulary.py`

Python is a thin host adapter to the typed Rust operations. Metadata uses the
existing durable action identity and consumes no generation slot. Protected
provider admission uses the existing host job/action identity. Receipt recovery
can reconstruct an eligible choice when the ephemeral NEXT slot is empty.

The inbox guard runs before file locking, archive moves or model-supply receipts.
Mike's ordinary messages are included in this deferral. Explicit CHECK_MAILBOX
ends protection and restores the existing receive policy; it does not author a
reply. Boot recovery does not insert a competing reflection or replenish the
focus budget. Existing authenticated stop, resource and authority checks retain
their paths; tests exercise the agent stop path without contacting a live model.

## Owner Transactions

`crates/astrid-source-study/src/owner_transaction.rs` provides the common
cross-process exclusive `reader.lock` with synchronous thread-local reentrancy.
The guard is deliberately not Send/Sync; no lock crosses inference or await.
The last nested guard owns release, including when guards are dropped out of order.

Reader, writer, native activity, saved-reader bookmark log and authoritative
selection participate. The saved-selector compare-and-swap revision protects
against stale in-memory callers even after lock acquisition. The old schema
guards remain necessary: an older binary does not participate in the new locking
contract. Never run old and new writers together during migration.

## Additional Privacy Repair

Source inspection found that `_run_shared_source_study` correctly retained
private writing in private files, but its general `_write_journal_entry` call
still copied the draft body into `sovereignty_journal`. The latest-entry prompt
query read the newest six rows without excluding private draft types.

New private-writing and private-writing-notice entries now bypass the general
database and content-based public diagnostic/afterimage hooks. They keep their
native/private artifacts unchanged. Generation-artifact links carry protected
metadata without passing draft prose to content matching.

The ambient latest-entry query excludes historical private-writing types but
does not delete or rewrite any existing database row. This is not a claim that
all historical exports have been audited or repaired. Synthetic tests verify no
general-journal insertion, no public hook invocation, exact artifact retention,
metadata-only linking and exclusion of retained legacy private rows from recall.

## Live Baseline Reconciliation

Selected historical bridge stage inspected:
`/Users/v/other/worktrees/source-study-evidence-20260917/bridge-stage-01`.

Manifest SHA-256:
`c2c40c356efa6758cac4e2f6e3e2b758d0b9963de36f11da5fd75e9c6fbc2a09`.
Source-input inventory SHA-256:
`1e4658959440ce45a10187eb838605dca95348beed3187dc62e7807263754622`.
Selected helper SHA-256 recorded by that release:
`02a973ca4329d9c57ac5b59971e1bbc81ef115f0f6d985c315907f37a160231a`.

The ten imported reader production/prompt files matched that immutable source
inventory before import: prompt.txt, lib.rs, notebook.rs, notebook_findings.rs,
notebook_persistence.rs, page.rs, source_search.rs, source_structure.rs,
store_navigation.rs and source_provenance.rs. Their diffs were read before
applying them to the isolated candidate. The relation regression suite was also
read and imported.

Preserved behavior: typed two-anchor authored relations; sticky relation-sidecar
v2 downgrade protection; syntax-based source-use labels with runtime uncertainty;
parsed consumer navigation; complete authored recall under the input ceiling.

The added focus guidance initially made the maximum escaped notebook fixture
exceed the unchanged 48,000-byte input budget. Optional generated suggestions
now yield before authored recall, and redundant standing prompt prose was
shortened. The source bytes, relation anchors, notes and question remain intact.
The exact maximum-size regression passes without raising the limit.

A full comparison covered 621 Astrid files from the live inventory. Remaining
non-owned differences were two historical backup artifacts, test-only additions
in action_continuity/tests.rs and autonomous/inquiry/parsing.rs, and six separate
reader reachability fixtures. They were not swept into this feature. There was
no additional unmatched production behavior outside the owned/imported paths.
This comparison is not itself a fresh deployment attestation.

Minime's reviewed runtime/parsing/visual_context files and tests matched the
qualified `/Users/v/other/worktrees/quiet-sensory-release-20260920/minime` tree:
- runtime.py: `bbb3f50c1386fd62b0e6919c5e62b7d9a046a6045bfeda58e18ed6ee2d2dfee6`
- parsing.py: `a00ce7be0d4abdbd294825b1b077a318f24ed7dc03397fb45f854e73224d0bd7`
- visual_context.py: `2dd0639dfa80a72ce94d57b818d4b8e5ad2e23d99dffa6d8ef2eaffee4cfd169`
- test_visual_context.py: `1e03f664f8ca146d1efd13edf8daeb276d4d8e025b2bba163cb662f6f68b8abd`
- test_source_study_notice.py: `df5c15a549e608cc5cf1aa064ea0a3cc78f6c35fe6fa093668b2959432cf1478`

The candidate retains fresh changed ambient-caption selection, separation of
visual prose from action lines, and receipt-gated source-study diagnostics.
Its stronger no-appended-advice rule supersedes the old notice-rendering test
expectation while retaining the diagnostic exemption.

## Verification and Failed Attempts

Passing checks:
- Shared reader/writer: 220 tests.
- Bridge library: 2,279 passed, one ignored.
- Complete Minime Python suite: 1,451 passed, one skipped, 136 subtests.
- Focused real-helper/runtime/visual/source-notice suite: 52 passed before the
  additional three privacy tests, which are included in the complete suite.
- Shared-reader and bridge all-target Clippy with warnings denied.
- Shared workspace and bridge formatting; diff whitespace checks.
- Domain-boundary audit: valid, zero violations.
- Controller/projector/event-store/Division suites: 77 passed.
- Introspection addressing self-tests: 44 passed; epistemic self-tests: two passed.

Tests use synthetic documents, isolated stores and stub providers. Python runs
with an allowlisted environment and the isolated Rust helper. No real-model
experiment was used as a test. Passing tests do not establish felt improvement.

Unsuccessful attempts, retained here:
- The first common-lock refactor omitted creating the writer directory; eight
  native tests failed with missing paths. Directory creation/permissions were
  restored under the lock and the suites rerun successfully.
- A reader-log failure fixture constructed a struct without its new transaction
  guard. It now opens the real log and substitutes a read-only failure handle.
- The maximum escaped relation fixture failed after adding focus guidance; first
  removing optional suggestions alone was insufficient. Compact standing prose
  plus bounded suggestion fallback passed without changing authored state.
- Two reviewed-diff patch attempts hit changed context; the failed patches were
  not treated as imports. Corrected scoped patches preserved the owned edits.
- Reader Clippy rejected a single-pattern match in the budget fallback; it was
  changed to if-let and lint rerun.
- Self-test CLI spelling differed between addressing and epistemic tools. Failed
  argument invocations were corrected; they were not counted as test results.

## Deployment and Git Boundary

No live stores were migrated. No service identity, configuration, helper
selection or stage pointer was intentionally changed. Readiness/telemetry checks
for a new release remain to be performed after an actual qualified activation;
historical process IDs are not presented as current verification.

Use only `scripts/build_bridge.sh` staging/activation and
`scripts/restart_minime_agent.py` for the eventual transition. The Minime wrapper
documents an observed idle gate, not an atomic old-agent drain handshake. It
reloads the existing canonical launchd job; it does not select this isolated
Python tree. Canonical-source reconciliation or a separately qualified immutable
selection mechanism is therefore required before invoking it. Do not mislabel
a restart of the old sources as rollout of this candidate.

Preserve engine, models, visual service, sensory clients, PI/damping/gains and
reservoir math. Never restore a backup over newer authored state, force a foreign
preflight, discard an uncertain completion or resume paused automations.

Both owned branches remain `codex/voluntary-continuity-20260920`, based on Astrid
`c4f85e95e41703daa65d3ce2789e1e5c46961c4e` and Minime
`5f4925f54580f1fd44666058b126a121ff32880f`. Indexes are empty; no commit SHA exists
for this tranche. Canonical dirty work was inspected again and left untouched.
Any future commit needs one coordinator, explicit paths and reviewed provenance
for the carried-forward baseline.

Controller status remains paused, generation 456, with no lease or active
projection. Evidence Event Store indexed-tail verification is valid at sequence
1123110, head
`7905070ce3ffee7955cbd97a716ddf1025b4ea4efe1260c8f6e47d9692e8d90f`;
all four V1 source hashes remain immutable. This interactive work recorded no
productive automation round and performed no source-first projection.

The separate September 20 offline continuity protocol remains a preregistration
only. Real-model execution and claims about persistent activations remain outside
this pass.
