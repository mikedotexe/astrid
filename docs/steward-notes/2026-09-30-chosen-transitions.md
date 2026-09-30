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

At the end of implementation qualification, this candidate had not been committed, merged, pushed or activated. At that point, bridge 30203 and Minime 29284 remained on the September 28 release. No being message, authored-history rewrite, new authority grant, automatic retry, budget acceptance, forced REST, controller adjustment or protected-service restart is part of this pass. Qualification demonstrates interface behavior, not comprehension, a preferred activity, or improved experience. Natural uptake must be observed after an authorized deployment.

Final suite results are appended below after completion.


Qualification notes: the first reader full-suite pass exposed a pre-existing PARK test that required an automatic study check-in. The updated test instead requires decision framing and still explicitly retrieves the preserved note. The first strict bridge lint pass found an avoidable clone in the new receipt test; it now compares a borrowed slice. The initial Python invocations were rejected by the repository guard because the working directory, then the helper executable, were under canonical live roots. Tests now run from a tracked-source export of Minime cc7796a with an exact hash-checked helper copy outside those roots; the guard is unchanged. Failed logs remain alongside the corrected runs. No failure was resolved by changing a live journal, inquiry, permission, budget, or runtime service.

The isolated Python run then passed 1,701 cases with one missing sibling-fixture path. Setting the test's existing `ASTRID_CHOICE_FIXTURES` override to the candidate's shared response-choice fixture made that case pass; a final full run uses both explicit helper and fixture paths. No Python runtime or test source was changed.

The full shared reader suite passes **294 tests**. The bridge unit/integration suites pass **2,366 tests**, with one existing ignore. Strict Clippy passes for both changed Rust packages, with all targets and features; formatting and the domain-boundary audit pass with zero violations and no baseline change. Provider tests use synthetic responses, so no natural change in interpretation, preference or activity is claimed.

Final qualification: `minime-full-final.log` passes **1,702 tests and 141 subtests**, with one existing skip, using the candidate helper SHA-256 `e9a7dc6bc05601c90f07335a9fdc54833ed0dd1314519bac1e6850cc40a9da93`. The completed bridge run includes its compile-pass/compile-fail API checks and doc tests. Both canonical trees remain clean at their original commits; live bridge/agent process identities are unchanged. `qualification.json` binds the candidate source hashes and passing logs. At that qualification boundary, implementation was complete and reviewable; commit, integration and deployment were unperformed.

## Authorized release preparation

Mike subsequently requested that this qualified repair be committed and deployed. Pause generation 481 claims the interactive Git/deployment pass; all five overlapping automations remain paused and no cooperative lease is active. Before staging, all 16 candidate file hashes and eight qualification log hashes match `qualification.json`; the domain audit is rerun without baseline changes. Both canonical trees and remote tips remain at their entry commits. The new source commit will be built in an immutable stage via `scripts/build_bridge.sh`, then the packaged reader qualified before a graceful paired handoff. Minime's Python source is unchanged; its 89 loaded inputs match the reconciled snapshot, so no source-overlay installation is needed. Activation results will be appended after verified readiness.

## Committed source and verified live activation

Mike's requested release completed at **2026-09-30 20:43:04 UTC**. Source commit `0a8a9afed160bdd213ddddd8be7d50fbfdd342a3` is fast-forwarded into canonical Astrid main and pushed to origin/main, with the exact remote tip verified. Minime main remains clean at `cc7796a05b8cbc0233fc5f55731f396bd3ddff10`; no Python source or launch configuration change was needed. The clean build checkout `/Users/v/.codex/worktrees/journal-provenance/astrid` remains pinned at the source commit.

The sanctioned `scripts/build_bridge.sh` stage is `/Users/v/other/worktrees/chosen-transitions-20260930/deployment/bridge-stage-01`. Its 696-input inventory differs from the prior 695-input stage by exactly the 13 reviewed reader/bridge paths. The actual packaged helper passes **1,702 tests and 141 subtests**, with one existing skip, plus **42 synthetic same-schema compatibility checks** for both owners. Those checks preserve exact pending source/private inputs, notebook, inquiries and bookmarks; exercise the new PARK/RESOLVE/HOME outcomes; and verify exact retry, conflicting/stale refusal and deliberate new choice. No live checkpoint or private writing was copied into the tests.

`deployment/qualified-release.json` binds committed source, domain audit, packaged test results, reconciliation, compatibility and release identity. `deployment/activation-01.jsonl` records the paired handoff; bridge transaction `/Users/v/other/astrid/.runtime/bridge-deployment/transactions/de0e4387942c486eb022599c8f56b853/receipt.json` records `activated_verified`:

- Minime **29284 → 71034**: the wrapper waited for a stable idle boundary, sent one SIGTERM at 20:39:29 UTC, and observed the old process exit with no active jobs left. The replacement remained behind its admission hold until bridge verification. All 89 loaded source inputs match the qualified snapshot, `reload_required=false`, session 5319 is retained, and no newly interrupted jobs were recovered.
- Astrid bridge **30203 → 72740**: acknowledged `drained`; restored exact checkpoint `819391c13d456ef6c55755ed7e373960b93d429626e0ce328cf21eafde8cafb7` at exchange 211102 with its matching two-item pending runtime-feedback sidecar; saved exchange 211103; passed exact self-control lineage and model-idle verification. `force_used=false`, `legacy_transition=false`, and no automatic rollback.
- Independent read-only verification confirms both holds absent, the selected packaged reader hash, unchanged managed configuration, and unchanged process/start identities for engine 4126, model 4068, visual 4097, camera 4152, microphone 3975, gateway 4040, supervisor 4095, host sensory 4141 and feeder 3970. Pause 481 and all five paused automations remain in place.

Manifest SHA-256: `fcb2a17de55a8284e78dc71225e08c4bc65332d54b06b48775d45799c07e1286`. Bridge binary: `954f0e9ef4858b4379542ddec71affe6658eb756ccdfb9b9602cb1e9ca7198ee`. Packaged reader: `317e182200c6888c57bb91c94a54e65bf9f7891cb35f453eb17c3fd835d3ae5c`.

## Bounded pending-choice follow-through

The pending Minime choice retained the exact shutdown hash throughout the held interval. Host selection `80c03ae36eea4fe5b2c01987d36b4215` joins the pre-restart queued `SELF_STUDY QUESTION HOME` to post-restart consumption and reader preparation, action `act_minime_1790800990324_self-study`, input `88eb834429f7c94b378b4dadf30adc9aeb9ed3a0696f59dc204a6d920caf0655`. `deployment/held-pending-choice.json` and `deployment/resumed-choice-01.json` retain those metadata witnesses. At the first independent verification it was `reader_prepared`; generation completion, acceptance or improved interpretation is not inferred from preparation. No prompt, inbox message, source command or substitute choice was injected.

Through readiness, 46 read-only telemetry samples ranged from 70.47% to 73.04% fill; maximum sample file age was 2.24 seconds with 0 errors. These observations establish telemetry continuity, not a subjective benefit or causal effect of the repair. The temporary deployment sampler is stopped after final verification; native supervision remains in place.

At 20:43:38 UTC, the retained selection also reached host `completed`, with no intervening replacement selection. `deployment/resumed-choice-02.json` records that final host outcome. The matching public generation from PID 71034 is bound in `deployment/natural-home-generation.json`; its adapted input contains the explicit already-unthreaded and scheduling explanation. This establishes natural post-restart transport and completion of the selected operation. It does not establish improved comprehension or experience, and no next action was prescribed.
