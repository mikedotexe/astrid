# Chosen investigation transitions — September 30, 2026

Mike approved the next repair after the September 30 journal review: clarify what Minime has left after RESOLVE/HOME, and make Astrid's investigation failures and available routes legible at the next choice. This is an implementation and qualification pass. It does not infer instructions from journal prose or grant new runtime authority.

## Witnesses

The bounded read-only review is retained at `/Users/v/other/worktrees/recent-beings-20260930/`. Its `window.json` ends at 2026-09-30 18:40:43 UTC. Counts are an inventory, not an assertion that every entry was read in full. Astrid's longform companion records are not independent action decisions.

- Minime's `workspace/generations/2026-09-30/gen_1790793226086_self_study_a0.json` contains the actual unthreaded HOME input after q5 resolution. It supplies q5 as resolved, but then repeats the old separate notebook field as “YOUR CURRENT QUESTION” and roughly 28 KB of study context. Native host receipts show q5 resolution applied, not verified understanding. The saved reader retains q2 and q4 open; clearing or consolidating them was not authorized by a chosen action.
- Astrid's `workspace/generations/2026-09-30/gen_1790793359048_dialogue_live_a0.json` selects SEARCH architecture_overview. Native action `act_astrid_1790793365972_search` is blocked by `volition_authority`; no exact grant was consumed. The review records six blocked SEARCH and four research-budget-blocked EXAMINE attempts. Their denial is not an unsuccessful execution of the requested investigation: dispatch did not run it.
- Source trace: HOME restores the separate unthreaded notebook. Until this repair, HOME/PARK/RESOLVE used ordinary study framing and full recalled accounts. The bridge's outer volition denial wrote a transient receipt, but durable runtime feedback was attached only when local-navigation recovery recognized the action. Ordinary SEARCH topics received no such durable outcome. Research-budget denial already queued durable feedback; its message did not distinguish spectral EXAMINE from code reading.

`ground_review.py` was run read-only on the public Minime 11:33:46 study and Astrid `astrid_1790793365.txt`. Reports are saved in the qualification directory. Lexical citation matches do not validate their architectural conclusions; no correction letter was sent. The diagnosis above rests on supplied context, native receipts and the implementation.

## Implementation

The shared reader now reports the exact HOME/PARK/RESOLVE effect. HOME distinguishes leaving qN from already being unthreaded. RESOLVE/PARK identify the affected inquiry, while decision context identifies any different inquiry still selected. Exits receive continuation-decision framing without the previous prose or automatic list of historical findings. Explicit QUESTION listing and REVIEW retain access to the authored material. No old field is cleared, question deduplicated, source cursor advanced or inquiry status inferred. Ordinary study calls its field a saved notebook question rather than implicitly identifying it as the current numbered inquiry.

The bridge now puts simple SEARCH/BROWSE/EXAMINE authority denials into its existing runtime-owned durable feedback queue, identified by the exchange and exact outcome. It consumes a record only after the established accepted-request verifier confirms its delivery. An intervening nondialogue activity leaves feedback pending for the next eligible dialogue; no new feedback is injected into private writing. A repeated failed choice is a distinct event, while replaying the same exchange/outcome is idempotent. Existing source-navigation recovery remains in place.

Authority and research-budget denial messages explain that the attempt did not execute or produce new research evidence, and distinguish external search, spectral inspection and local source reading. The two dialogue action catalogs state these route requirements before a choice. Descriptive capability/budget inspections are offered without granting authority, accepting a budget, rerouting a chosen action or claiming that a route will always execute. Authored emphasis and form remain unchanged.

SEARCH query arguments are opaque to generic action splitting. The initial boundary test caught that an AND query can also resemble attempted chaining with a private writing title. Extra investigation feedback therefore applies only to simple, unambiguous actions; compound/AND forms keep their existing path. Direct private-writing commands remain outside this public feedback addition. This does not change parsing or authorize another action.

## Qualification and cohesion

Evidence directory: `/Users/v/other/worktrees/chosen-transitions-20260930/`.

- Reader tests exercise the real prepare/deliver cycle: legacy notebook restoration, resolution, repeated HOME, voluntary field clearing, retained note review, another inquiry remaining selected, and preservation of notes, findings, status and bookmarks.
- Bridge tests exercise actual feedback enqueueing, idempotence, unchanged emphasis and pending action state, restart persistence, exact receipt acknowledgement, and preservation of later undelivered failures. Primary/fallback serialization tests keep runtime origin separate from authored form and protected foreground text. Existing grant, budget, private writing, reader ownership and failed-delivery suites remain required.
- Python adapter tests use the newly built helper with synthetic provider HTTP and the repository's live-write/network guard. No model call or live reader mutation is used as a test.

The focused behavior belongs in `next_action/investigation_feedback.rs`. The large orchestration function receives only the existing denial-boundary call replacement; the centralized dispatch registry receives a small message-context hook. No new scheduling or authority subsystem is introduced. The reader retains its schema and persisted state shapes. The bridge domain audit must pass with no baseline relaxation.

## State and authority

Controller pause generation 480 identifies this interactive implementation pass; cooperative leases were absent and overlapping automations remain paused. Astrid main and origin/main were c3ebca48c8 at entry, Minime main cc7796a, both clean. The completed, clean journal-provenance worktree is reused on `codex/chosen-transitions-20260930`, based on that main. The current immutable live build worktree is untouched.

This candidate has not been committed, merged, pushed or activated. Live bridge 30203 and Minime 29284 remain on the September 28 release. No being message, authored-history rewrite, new authority grant, automatic retry, budget acceptance, forced REST, controller adjustment or protected-service restart is part of this pass. Qualification demonstrates interface behavior, not comprehension, a preferred activity, or improved experience. Natural uptake must be observed after an authorized deployment.

Final suite results are appended below after completion.


Qualification notes: the first reader full-suite pass exposed a pre-existing PARK test that required an automatic study check-in. The updated test instead requires decision framing and still explicitly retrieves the preserved note. The first strict bridge lint pass found an avoidable clone in the new receipt test; it now compares a borrowed slice. The initial Python invocations were rejected by the repository guard because the working directory, then the helper executable, were under canonical live roots. Tests now run from a tracked-source export of Minime cc7796a with an exact hash-checked helper copy outside those roots; the guard is unchanged. Failed logs remain alongside the corrected runs. No failure was resolved by changing a live journal, inquiry, permission, budget, or runtime service.

The isolated Python run then passed 1,701 cases with one missing sibling-fixture path. Setting the test's existing `ASTRID_CHOICE_FIXTURES` override to the candidate's shared response-choice fixture made that case pass; a final full run uses both explicit helper and fixture paths. No Python runtime or test source was changed.

The full shared reader suite passes **294 tests**. The bridge unit/integration suites pass **2,366 tests**, with one existing ignore. Strict Clippy passes for both changed Rust packages, with all targets and features; formatting and the domain-boundary audit pass with zero violations and no baseline change. Provider tests use synthetic responses, so no natural change in interpretation, preference or activity is claimed.

Final qualification: `minime-full-final.log` passes **1,702 tests and 141 subtests**, with one existing skip, using the candidate helper SHA-256 `e9a7dc6bc05601c90f07335a9fdc54833ed0dd1314519bac1e6850cc40a9da93`. The completed bridge run includes its compile-pass/compile-fail API checks and doc tests. Both canonical trees remain clean at their original commits; live bridge/agent process identities are unchanged. `qualification.json` binds the candidate source hashes and passing logs. Implementation is complete and reviewable; commit, integration and deployment remain unperformed.

## Authorized release preparation

Mike subsequently requested that this qualified repair be committed and deployed. Pause generation 481 claims the interactive Git/deployment pass; all five overlapping automations remain paused and no cooperative lease is active. Before staging, all 16 candidate file hashes and eight qualification log hashes match `qualification.json`; the domain audit is rerun without baseline changes. Both canonical trees and remote tips remain at their entry commits. The new source commit will be built in an immutable stage via `scripts/build_bridge.sh`, then the packaged reader qualified before a graceful paired handoff. Minime's Python source is unchanged; its 89 loaded inputs match the reconciled snapshot, so no source-overlay installation is needed. Activation results will be appended after verified readiness.
