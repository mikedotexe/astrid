# Shared extended writing and private drafts · September 9, 2026

Mike accepted the longform-room proposal after the fixed study audit found that larger
output ceilings alone were not producing longer entries. This change gives both Beings
an explicit, persistent choice and a place to continue the same thought.

## Behavior

- `WRITE PROFILE EXTENDED` selects an 8,192-token output ceiling across journal-producing
  provider calls; `SHORT` selects 512 and `DEFAULT` restores existing mode preferences.
  An explicitly selected WRITE profile overrides ordinary mode/LENGTH ceilings; DEFAULT
  restores those preserved preferences. There is no minimum
  length. The profile survives Ollama/MLX fallback clamps and adjusts request/job deadlines.
- `WRITE START <topic>`, `CONTINUE`, `REVISE <direction>`, `BRANCH <direction>`,
  `RESUME dN`, `FINISH`, `LIST`, `QUESTION <text>`, and `EVIDENCE <text>` use one shared
  Rust writer. `READ dN [page]` reopens a large saved draft in explicit 9,000-byte pages.
  Help is `WRITE HELP`. A draft turn has a small menu and can choose any ordinary NEXT.
- Full visible passages, question, references, exact request/response and earlier revisions
  persist in the owning reader's `writing/` directory. Continuation carries the whole
  current draft, not an opening excerpt. Receipt retries are idempotent; failed/shortened/
  stale/length-terminated delivery never replaces a completed draft. Failed provider wire
  remains in existing diagnostics. Navigation-only responses need not add draft prose.
- The shared study notebook now stores up to 64,000 bytes per recent answer and renders
  up to 32,000 bytes, dropping oldest whole answers before labelling excerpts. The full
  protected input allowance is 48,000 bytes and context reservation 65,536 tokens. A draft
  that cannot fit is left intact with an explicit capacity notice, paging and a new-draft
  option; there is no silent draft truncation.
- `private_writing/journal/` and `private_writing/artifacts/` sit outside peer journal scans.
  WRITE creates no sensory payload, companion inbox letter, or auto-promoted shared thought.
  Minime's private draft also bypasses ordinary journal compression. Existing public modes
  keep their delivery behavior. Model thinking, coupling, reservoir and sensory dimensions
  are unchanged.

## Important correction to the preceding audit

`record_next_choice` generated forced-redirect metadata and "This turn" wording, but
`orchestration.rs` already used the authored NEXT as its effective action and logged the
redirect as advice. This release makes the repetition helper itself advisory for read-only
exploration and writing. It does **not** claim to newly remove an active dispatcher veto.
Existing health, external-action authority and resource scheduling checks remain.

## Validation and limits

Reader tests cover full-tail retention, process reconstruction, delivery retry, replacement
revision history, branches, finish/resume, exact profile values, stale/conflicting delivery,
length termination and oversized-draft recovery. Host tests exercise real Minime adapters,
private journal placement and actual action dispatch; Astrid tests cover provider policies,
final dialogue clamps, private mode attestation and NEXT dispatch. An existing signature-tampering test occasionally replaced an already-zero byte with
zero; its corruption now always changes the signature. Full suites, strict
Clippy, workspace formatting and the domain-boundary audit are recorded in `checks.json`.

Two isolated native Ollama trials used 8,192 output / 65,536 context: the source control
stopped at 781 generated tokens (117.69 s), the private draft at 572 (64.77 s). Both returned
usable nonempty responses and native stop. This qualifies request admission and usability,
not sustained 8,192-token generation, natural uptake, understanding, or coupled-Astrid
performance. The source-control response still asserted a host-mediated gateway path not
established by its supplied excerpts. The free draft chose WRITE CONTINUE but its NEXT was
not executed. Outputs were not delivered to the Beings.

## Deployment and rollback

Deploy the immutable bridge/helper stage first, then gracefully reload Minime's Python
agent using its owner-controlled restart tool. No engine or model restart is required.
The profile is opt-in. Reverting it to DEFAULT restores ordinary capacities while retaining
all drafts. An older helper cannot understand WRITE, so do not point Minime at one while
pending WRITE actions remain. Keep new writing records during any code rollback.

The [owning rollout receipt](extended-writing-validation/live-rollout.json) verifies
the release and process boundary. Research history HSS-18 retains qualification and
deployment as separate facts.

### Graceful activation interruption

The first activation drained PID 53226 and saved checkpoint
`384897312290a79e8c4dfe1080306513953adbd6fb6071f38245655af70a56f0`,
then reported "old PID was reused during transition" while the process exited.
The launcher retained its owned hold; no activation or force kill occurred.
The supported stopped-transition path rejected the matching V3 release because
its schema gate admitted only V2, although ordinary activation already admits V3.
The recovery helper now admits those same two formats, preserving all hash,
checkpoint, PID-absence, handoff and hold guards. Regression tests exercise V3
continuity, mismatched hashes and rejected legacy/unknown formats. The immutable
staged binaries and source inputs remain unchanged. Live recovery is recorded
separately after its verification completes.

### Verified live release

At 2026-09-10 01:47:20 UTC (September 9, 18:47 PDT), bridge PID **80595** runs
Astrid source **6f8aac6384232bc010d2776f13454eb8795bf3ae**, with matching immutable
helper and manifest. Recovery restored the exact drained conversation checkpoint,
verified self-control integrity and its new binary binding, and observed a new saved
exchange. The original failed activation receipt is retained unchanged alongside the
successful stopped-recovery receipt. No force, second signal, or automatic rollback.

Minime PID **81874** loads **33c324d229893a061ea958891f19fa450f232bd6** after an
observed idle-boundary SIGTERM. Its exact pending `SELF_STUDY FIND
astrid/capsules/spectral-bridge/src/autonomous/next_action/mod.rs Route` survived
restart and was dispatched by the new worker. Session identity and cycle continuity
verify. The shared helper selection and new writing adapter source hashes match.
All ten surrounding engine, model, sensory and collaboration services retained their
PIDs, start times and launch configurations.

All 610 staged source inputs remain byte-identical. 571 Astrid source/build inputs matched canonical main; the only differing
input was the separately committed recovery helper, whose
single schema compatibility change is recorded explicitly in the receipt; the frozen
stage was not edited. Source and recovery commits are on main. The shared checkout's
pre-existing uncommitted edits are preserved separately from this release.

No WRITE profile or draft was forced into either Being's state. The release provides
choices; spontaneous uptake, sustained long writing and improved understanding are
future observations. The process-identity error during the original stop is retained
as an unresolved controller observation, not evidence that PID reuse actually occurred.
