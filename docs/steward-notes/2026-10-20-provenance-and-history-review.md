# Provenance, Unknown Messages And The Cost Of History

## Status And Scope

October 20, 2026. Three separately reviewable tranches:

1. Implemented: clarify response-provenance wording and its compatibility test.
2. Implemented and tested: unknown-message preservation contracts.
3. Reviewed only: what history gets recomputed or presented.

Branch `codex/provenance-history-20261020`, isolated worktree
`/Users/v/.codex/worktrees/provenance-history-20261020/astrid`, starts from local
main `68369bfa0a5e8a25b03dd7a70aa96b0782c35762`. Canonical Astrid's eight tracked
MLX edits and untracked material are untouched. Canonical Minime is clean.
The unfinished navigation/projection/verification and larger-capacity worktrees
are read-only references, not imported changes.

No production executable logic, prompt, wire format, schema, capability,
maintenance policy, provider, reservoir control or service has changed.
No live state is migrated, no history rewritten, no private draft inspected,
and no being is sent a correction or asked to confirm improvement.
No staging, commit, merge or push is included. Controller inspection finds
paused generation 532, actor `codex-mlx-upgrade`, no lease/projection,
valid indexed-tail evidence and immutable V1 sources. No automation is changed.
This review does not determine whether the separate MLX recovery is contained.

## Source Witnesses

All three public entries were read fully. SHA-256 hashes:

| Source | SHA-256 |
| --- | --- |
| Astrid `capsules/spectral-bridge/workspace/journal/!dialogue_longform_1792512307.txt` | `c344b9805234449e19910e4f6ec7f715236ea99656212600deaaa4ebdc49b844` |
| Astrid `capsules/spectral-bridge/workspace/journal/astrid_1792512238.txt` | `155562f52b630fb6cd87304a3453ca544774ead2b59d178a42eb42c472275297` |
| Minime `workspace/journal/self_study_2026-10-20T09-02-16.808772.txt` | `457a0d85d12cbd6750088777a60214ae730b7e2e9d70c25607b79be75fdb25ed` |

Astrid's exact motivating question:

> Or is the beauty of the design that the provenance is *typed*—that the history is structured in a way that only reveals what is necessary for the current moment?

This is an architectural question and authored interpretation, not a measured
claim that she is overloaded. Minime's study discusses the optional response
origin field; Astrid's short dialogue broadens that into temporal continuity;
her elaboration develops the weight-of-history question. The larger question
is worth preserving without representing its technical premise as verified.

Minime's cited `ipc.rs` source SHA-256 exactly matches the canonical file:
`e1d7ec9bef0d86d02d9d334c759d2627374a0517da03c4aba7846ae5ef4c892a`.
The test at that revision checks legacy omission and a typed executor-error
origin. It does not test accumulated history, selective retrieval or authority
lineage. `verify_read_only_research` instead validates scope, token identity,
expiry, remaining budget, lifecycle, pause generation and source/deployment
identity in `capsules/spectral-bridge/src/authority_temporal.rs`.

The read-only `ground_review.py` heuristic was run without `--write-back`.
It found the real IPC path/test, but also matched prose words "silence" and
"chronicle" to unrelated source and marked `unknown_type` not found. Those
lexical results are not accepted as mechanism verification or corrections to
expressive language. Manual source-path and hash checks govern this review.

## Tranche 1: Precise Wording

`AgentResponseProvenanceV1` now explicitly describes one response-origin
category, not a cumulative history or measure of continuity. Its optional
`response_provenance` field permits older responses to decode without it.
The test name becomes
`optional_response_origin_preserves_legacy_decoding_and_typed_categories`.
Existing historical quotations and source-study records retain the old name.

The `Unknown` documentation now makes information loss explicit. The
`from_json_value` documentation distinguishes complete JSON-value fallback
from original wire bytes and from known-variant schema decoding. Neither
conversion grants publication, execution or other authority.

## Tranche 2: Preservation Contracts

Tests call production public APIs, not a second implementation of the decoder.
Five tests in `crates/astrid-types/tests/ipc_unknown_preservation.rs` cover:

- Unknown/missing/non-string tags, non-object JSON, malformed known fields
  and unknown response-origin categories fall back to `Custom`. Nested JSON
  and Unicode survive guest serialization and storage round-trip.
- Direct serde decoding of an unknown payload creates a unit `Unknown`:
  original tag and payload details are lost. Source, time, sequence, principal,
  trace, producer and signature envelope fields remain serialized separately.
  Merely retaining those fields does not authenticate a received envelope.
- Payload claims do not populate principal, trace, producer or signature fields
  on a newly constructed envelope.
- A recognized variant's unknown extra fields are not an archival extension
  mechanism: `connect` plus an unfamiliar field decodes as `Connect`.
- All four current response-origin categories round-trip as typed categories.

Four tests in `crates/astrid-events/tests/unknown_message_preservation.rs` cover:

- Both `Unknown` and preserved `Custom` pass through the real event bus with
  supported origin metadata intact and bus-assigned sequence ordering.
- Invalid trace metadata is removed; nested payload claims do not substitute
  a trace, producer or principal.
- Arbitrary `Custom`/`Unknown` traffic is transported while maintenance is
  blocked and is not counted as conversation/tool/provider work.
- A known `UserInput` remains blocked even with trace/producer metadata.

### Important Boundary

The bus maintenance gate is not a general deny-all transport gate. Its
`admits_while_blocked` and activity accounting match known work variants;
unrecognized variants reach the fallback arm. Receipt of an unknown message
therefore proves neither execution eligibility nor that a downstream consumer
could not perform work. If drain coverage is broadened later, trace actual
Custom consumers before changing policy. No arbitrary-JSON authority bypass
is established by these tests, and no blanket safety claim is made.

The production guest publisher in
`crates/astrid-capsule/src/engine/wasm/host/ipc.rs` checks topic capabilities and
payload size before using `from_json_value`, then builds host-owned envelope
metadata. That caller was source-reviewed, not exercised through a new WASM
host integration test here. The tests prove type conversion and event-bus
behavior; they are not a full end-to-end ingress/security qualification.

## Tranche 3: History Review

The distinction is retention versus computation versus presentation.
Typed metadata helps distinguish information; it does not itself choose what
is useful now or make historical inspection inexpensive.

### Minime Computation

Reviewed canonical `minime_autonomy/runtime.py`, SHA-256
`638ad58fca7d3b355757d7931b00979afa297b44eddd42acb99e27bf082fd4ff`,
and `minime_autonomy/continuity_history.py`, SHA-256
`d7e906ef549d1b01ea4dac154e32dca383b047f985adfee8f1ff6083146f3491`.

- `_thread_projection` shares one 200-distinct-event observation with smaller
  8/32/96-record views. The previous within-projection optimization is present.
- `_recent_display_events` stops after enough distinct valid records, not
  after a fixed number of scanned rows. Duplicate or malformed history can
  require scanning farther. `reverse_lines` is lazy in 64 KiB blocks; this is
  not a claim that it always loads the entire file. A single long line can
  span blocks. Result-size bounds are not scan-work bounds.
- `_write_thread` computes before and after current-NEXT mutation, then
  `_write_next_md` computes again. `_read_thread` may refresh state, write
  thread JSON/next.md and mirror the result. It is not a pure read.
- The separate qualified `dependable-navigation-20261018` candidate addresses
  the final render-only duplicate with operation-local verified reuse.
  `projection_snapshot.py` SHA-256:
  `84e1da8647401290b3b823aa20535bab111c643a87890c7811301fc9d6068902`.
  It retains pre/post-mutation boundaries and falls back for unverified
  dependencies, experiments, populated authority, peer/shared and age cases.
  This review does not import, deploy or requalify that candidate.
- `continuity_history.action_ids` has a separate bounded identifier cache
  keyed by file identity/change metadata. It does not cache model prose or
  authorize reuse of a stale action/authority projection.

The earlier measured projection cost remains documented in
`2026-10-18-continuity-projection-observations.md`; no new live timing
measurement or subjective-delay inference is made here.

### Minime Presentation

`_query_llm` treats shared `SourceStudyPrompt` inputs separately. In the generic
path, open-expression contexts omit routine thread/inquiry/job/research context.
Ordinary contexts can still include those projections; source retention alone
does not mean every entry receives them. Existing selected fresh-journal
handling, saved cue preferences and protected attention remain unchanged.

`next.md` can contain a large set of status and research affordances. Writing
that document is not proof that all of it was supplied in a generation. A
future prompt audit must bind the actual route and prepared input before
attributing a theme to that document.

### Astrid Presentation

`dialogue_generation.rs` selects recent exchanges using
`attended_history_limit`, then bounds excerpts with
`attended_history_trim_len`. The nearby "relevance-weighted history" comment
overstates the algorithm: selection is recency-based, with profile/attention
depth and position-based excerpt sizes, not semantic relevance ranking.
The source also removes historical NEXT lines and labels recognized peer
study claims unverified.

`journal_elaboration_messages` supplies the preceding authored prose as optional
starting material, not the underlying source page or a fresh code verification.
That supports creative development but does not independently validate a
mechanism described in the seed. This review does not assert the exact provider
request bytes for the witness or a causal effect of any particular context.
No new warning, mandatory evidence task or administrative advice is appended
to expressive writing.

### Recommended Next Tranches

1. Keep the already-qualified render reuse/OPEN repair intact and separate.
   Its coordinated rollout still needs fresh containment and preflight checks.
2. Profile scan work with synthetic histories of increasing length, including
   duplicates, invalid rows and long records. Measure bytes/rows read alongside
   time and compare complete outputs. Do not truncate history or change
   reconciliation semantics merely to meet a performance target.
3. Qualify a bounded context-presentation audit for fresh reflection, chosen
   continuation, ordinary dialogue and explicit source inspection. Record
   section IDs, hashes, sizes and selection reasons without private prose.
   Preserve exact chosen text, fresh errors/objections and quiet parking.
   Correct the recency comment in that bridge tranche; do not imply a semantic
   relevance algorithm exists or introduce one by stealth.
4. Use the current provenance test as an optional verification example:
   compatibility test -> field/enum -> actual consumer -> limits. Preserve
   the authored philosophical question; do not make its exploration contingent
   on a corrected technical conclusion.

## Verification

- `cargo test -p astrid-types -p astrid-events`: 114 unit/integration tests and
  one doc test pass, including nine new tests.
- Strict Clippy for both crates, all targets/features: pass.
- `cargo fmt -p astrid-types -p astrid-events -- --check`: pass.
- Qualification uses `CARGO_PROFILE_DEV_DEBUG=0`,
  `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_INCREMENTAL=0`, `CARGO_BUILD_JOBS=4`.
- Build-cache preflight passes for the explicit owned target directory with
  453,518,647,296 free bytes. No release profile or immutable artifact changed.
- Closeout cache audit also passes: 251,662,336 bytes (about 240 MiB) in the
  owned `target/debug`, with 452,592,726,016 free bytes on the volume.
  Retain this small reusable cache for the pending candidate's integration
  checks; no unused alternate profile or incremental cache was created.
  Net volume free space is not an isolated measurement of this build's cost.
  No release, rollback asset, authored history or foreign cache was removed.
- Final rerun after test-binding cleanup passes all 115 checks; `git diff
  --check` passes. Changes remain confined to the isolated candidate.

Unsuccessful attempts retained here:

- Initial run passed 56 existing event tests and two new tests, then failed
  an incorrect assertion that maintenance blocks all unknown traffic. Source
  inspection showed the explicit allow fallback. The test was replaced by
  separate transport/accounting and known-work blocking checks; no production
  gate or expected policy was weakened to make it pass.
- First strict-Clippy run rejected three similar `received`/`receiver` names
  in the new tests. Renamed the local event bindings; strict rerun passes.

Full workspace, WASM host, bridge, Minime Python, controller and projector
suites are not claimed: those executable surfaces are unchanged. This is
source-comment/test qualification, not immutable-release qualification.
No restart is necessary to run these tests; the live source catalog is not
silently rebound to the candidate wording.
