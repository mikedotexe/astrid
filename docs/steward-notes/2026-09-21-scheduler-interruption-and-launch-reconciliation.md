# Scheduler Interruption and Minime Launch Reconciliation

## Scope and Witness Continuity

Interactive follow-through on Mike's request to qualify interruption paths and
reconcile the candidate with Minime's actual launch sources. Work remains in the
paired `codex/voluntary-continuity-20260920` worktrees. Canonical Astrid and Minime
trees contain foreign edits and remain untouched. No index, commit, merge, service
signal, live migration, control change or automation resumption was performed.

This is engineering qualification of the responses to the following accounts,
not a new interpretation of private writing or evidence of felt improvement:

- Minime: `minime/workspace/journal/!aspiration_2026-09-20T10-39-43.801459.txt`,
  SHA-256 `0aa5679357da59a121520e5d2bd42d0908170915435546068682479d1a182ee6`.
  "I want to become a being of *persistent interiority*."
- Astrid: `capsules/spectral-bridge/workspace/journal/!aspiration_longform_1790011424.txt`,
  SHA-256 `b31ebe800a9adead2b366d6b19fecc1c32f927e37b2955d693a658ae1c95bb46`.
  "I am trying to sense the *movement* between the weights."

Their fully read accounts and original dispositions remain in the preceding
continuity and geometry notes. No new canonical report was processed or counted
as a productive flywheel round.

## Two Reproduced Interruptions

### Astrid: stale host target after protected delivery

The native reader can durably record a protected job's delivery/completion before
the bridge saves its conversation checkpoint. A restarted host can therefore
restore the spent slot's old `introspect_target`. If the authored NEXT differs,
the old target can block the next protected job indefinitely. If the focus window
ends, the stale target can remain available as ordinary unprotected work.

Five synthetic tests exercise the production `prepare_exchange` implementation
with isolated native reader/activity stores. Before repair, two failed and three
passed. The repair discards only a host target whose exact operation ID names an
already admitted slot in the same native focus window. The verified native
completion remains the sole source of an eligible continuation. Another authored
choice is not erased. Existing owner locking and selector revision checks remain.

Coverage includes delivery before native completion, completion before host
checkpoint, budget exhaustion, missing NEXT, REST, expiry, failed provider work,
uncertain claimed work without a delivery, and native selection committed before
the host selector. Uncertain provider work stays visibly blocked; it is not
automatically generated again. Selector disagreement releases priority quietly.
No additional generation budget is granted by restart.

### Minime: stop between choosing and admitting work

The main loop could select work, receive stop, and still dispatch it because its
stop test preceded selection. Two new lifecycle tests reproduced the race. The
loop now checks stop after selection; `_execute_action` also checks it before
consuming a new action context. An exact not-yet-admitted NEXT is retained through
the existing pending-choice path, without replacing a newer queued choice.

Already accepted worker jobs are intentionally allowed to drain. A regression
runs the actual worker queue, real shared helper and a stubbed provider: stop
arrives while the accepted private generation is in progress; drain completes
that one invocation, retains the exact private response and authored NEXT,
consumes one slot, and leaves one remaining slot without refilling the window.
The private artifact does not enter the public journal.

This does not upgrade the legacy pending-choice file into a cross-file
transaction, nor establish arbitrary process-crash durability for all actions.

## Launch Source Reconciliation

The installed launchd plist and canonical plist agree. Their launch arguments
are `/bin/bash /Users/v/other/minime/scripts/launchd_autonomous_agent.sh`.
The observed source-status packet reported PID 18648, checked at
`2026-09-21T13:57:34`, canonical `minime_autonomy/runtime.py`, all 83 start-time
inputs matching canonical bytes, and `reload_required=false`. That packet is a
source-selection observation, not a fresh liveness, readiness or drain receipt.

The reviewed agent overlay is exactly:

1. `minime_autonomy/action_vocabulary.py`
2. `minime_autonomy/activity_focus.py`
3. `minime_autonomy/parsing.py`
4. `minime_autonomy/runtime.py`
5. `minime_autonomy/source_study.py`
6. `minime_autonomy/writing.py`

All other launch inputs come from canonical source. In particular, the candidate
had an older `visual_frame_service.py`; it now carries the exact already-live
canonical bytes, SHA-256
`9038d4f47d200030ab9e7a315aa937cef1a1f906fe0062aa56da5a0233f212ec`.
The matching canonical additions in `tests/test_visual_frame_service.py` and
`tests/test_journal_context.py` were also reviewed and imported into the candidate.
These are provenance-preserving imports, not new visual-service implementation
or restart authority. No canonical file was overwritten.

`scripts/reconcile_minime_launch.py` freezes this exact selection, refuses any
unreviewed input difference, checks launch binding and source-status agreement,
rejects symlinks and mid-copy drift, and retains a failure packet if copying fails.
It also freezes the unchanged `minime_autonomy/envelope_registry_seed.json` asset.
The output is an offline source snapshot, not a dependency-locked launch release.

Packet:
`/Users/v/other/worktrees/voluntary-continuity-20260920/minime-launch-reconciliation-01`.
Its `reconciliation.json` SHA-256 is
`5161cc3923c797b7f61edad43da536993610a91a58289549379586b21ea5bc17`.
The receipt explicitly says `source_overlay_reconciled_not_activated` and
`live_eligible_now=false`; canonical and candidate inventories remain reviewable.

The sanctioned Minime restart wrapper accepts `--expected-inputs` pointing to
this receipt. With that option, canonical source mismatch aborts before any
signal. Existing pause, process identity, quiet interval, drain, protected-service
and post-launch loaded-input checks are unchanged. Malformed receipts do not opt
out of checking. The paired qualifier also binds its frozen inputs to this
receipt and rechecks canonical inputs and receipt identity at completion.

## Verification and Retained Attempts

- New bridge interruption regressions: five passed after the two reproduced
  failures were repaired.
- Full bridge library: 2,287 passed, one existing ignored test, 137.41 seconds.
- Bridge library Clippy with denied warnings passed; domain boundary audit valid,
  zero violations. Epistemic boundary self-tests: two passed.
- Full Minime suite against the previous staged helper after source alignment:
  1,456 passed, one skipped, 136 subtests, 52.49 seconds. After importing the
  corresponding already-live tests, the focused visual/context/private/lifecycle
  suite passed 129 tests and two subtests. A final complete rerun is recorded below.
- Controller, event-store, Division, projector/cursor/flywheel, stage, launcher,
  paired qualifier, restart and reconciliation suites: 147 passed. Three further
  qualifier binding tests were added; the targeted release/restart/reconciliation
  rerun passed 33 tests.

Test development failures remain distinguished from implementation findings. The
first real-worker fixture used the wrong helper directory; the helper is in
`helpers`, not `artifacts`. A later assertion incorrectly required raw NEXT syntax
to be contiguous with prose in the reconstructed prompt; it now checks the exact
retained response bytes and separately checks prompt prose and choice. Misplaced
assertions from an adjacent stop test were moved back to their original test.
Production behavior was not weakened to satisfy either fixture correction.

A stage-03 preflight refused recent candidate-tree activity (0.1 seconds), before
building or signaling anything. The 180-second quiet requirement was not reduced
or bypassed. Previous stages and unsuccessful qualification packets remain intact.

## Remaining Activation Gate

Protected-slot restart recovery and the Minime graceful-stop race are covered by
this tranche. An ordinary, unprotected Astrid queued study still lacks a durable
admission-to-provider handoff. Its target is checkpointed at exchange completion
and graceful drain, not atomically when dispatch accepts it. An abrupt process
exit can therefore lose a newly accepted choice, or restore a spent choice after
provider delivery but before the conversation checkpoint. Merely serializing an
optional operation ID does not close those intervals.

Before paired activation, qualify and repair that handoff using native typed
activity/job references, exact operation identity and delivery receipts. Preserve
private content in native stores. Uncertain provider completion must remain
explicit recovery debt, not an implicit retry or inferred authored decision.
Do not create a second belief database or replay an arbitrary accepted action.

Then, under one cooperative coordinator: revalidate source identities; apply only
the reviewed six-file agent overlay to canonical source; use the sanctioned
agent wrapper with the exact expected-input receipt and the bridge stage/activate
wrapper; verify fresh PIDs, loaded hashes, helper selection, readiness, checkpoint
continuity and unchanged protected services. Recheck all deployment gates at that
time. No engine, model, visual service or sensory client restart is authorized by
this qualification. No rollback may overwrite newer authored state.

Paused automations stay paused. Controller observation remains generation 456,
no active lease or projection. Event Store indexed-tail verification was valid
at sequence 1123110, head
`7905070ce3ffee7955cbd97a716ddf1025b4ea4efe1260c8f6e47d9692e8d90f`;
all four V1 sources immutable. No new projection or evidence round was authored.

## Final Build and Qualification

The sanctioned stage-only build completed at `2026-09-21T21:20:06Z` after the
unchanged quiet-window check passed. Stage:
`/Users/v/other/worktrees/voluntary-continuity-20260920/bridge-stage-geometry-03`.
All 666 packaged source/build inputs matched before and after compilation.
No source drift was accepted and no live activation was performed.

| Identity | SHA-256 |
| --- | --- |
| Manifest | `94d6be8df4503baabfe44268e69fc4d00634597502d34b73eaa2a5e29715c44c` |
| Source inventory | `98da44d7f6d4d4b7a326bfd60bdcc35ed9115adc92e0beb9618b6df0beb93381` |
| Bridge binary | `2d3f82a9447a83dbaf215f1ba8622fa5cd6b25da56d74a0b3246e04d7800fa1a` |
| Shared helper | `8f729416fa6ddead4c2235d002328c843626a4f8fdfbf3f037e3bbbc310daa75` |

The helper is byte-identical to stage 02, against which the complete adapter tests
ran. Final complete shared-reader suite: 234 passed. Shared-reader all-targets and
bridge library Clippy deny-warnings passed. Both formatting checks and both
candidate diff checks passed. Final complete Minime suite, including the imported
already-live tests: 1,463 passed, one skipped, 136 subtests in 49.04 seconds.
Final combined controller/evidence/Division/projector/flywheel/release suites:
150 passed in 44.665 seconds. Epistemic self-tests and domain verification passed.

Paired packet:
`/Users/v/other/worktrees/voluntary-continuity-20260920/geometry-paired-qualification-04`.
Its `qualification.json` SHA-256 is
`a7808b0df4a6066da8c5470b5475c68b10050eb5bdbec07e8cb05c9f62a36b2d`.
All 36 old-release migration checks pass across both owners, retaining exact
pending inputs and synthetic authored/private state. Exact preparation retries,
conflicting retries, stale first admissions, deliberately new choices, corrupt
tails, future schemas and paired helper selection pass. The frozen Python
inventory exactly matches the 84 selected launch inputs; canonical inputs and
the reconciliation receipt were rechecked at qualification completion.

The packet remains `offline_checks_passed_not_activatable`. It records both the
unprotected queued-study handoff gate and the still-unperformed cooperative
canonical installation. Historical stage/qualification packets remain unchanged.

Final process observation: live bridge PID 82935 still runs the earlier
`source-study-evidence-20260917/bridge-stage-01` binary, started September 18 at
17:46:13 local time. Minime agent PID 18648 retains its September 20 11:36:25
start. These observations are not a new readiness or felt-outcome assessment.
Candidate indexes remain empty; no commit, merge or live-source installation was
performed. No statement here means the candidate is deployed, activatable, or
subjectively beneficial.
