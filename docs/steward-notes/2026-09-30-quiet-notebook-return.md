# Quiet Notebook Return and Truthful Study Decisions

## Status and Ownership

Implementation candidate, not deployed, committed, merged or pushed. This is an
interactive follow-through to Mike's approval, not a resumed automation round.
Paired isolated worktrees at `/Users/v/other/worktrees/quiet-study-return-20260930/`
use branch `codex/quiet-study-return-20260930`. Astrid base: `d136ffc36b`;
Minime base: `cc7796a05b8cbc0233fc5f55731f396bd3ddff10`.
Canonical worktrees were clean before work. Previously paused automation remains
paused; no live checkpoint, journal, question or service was edited.

## Witness and Diagnosis

Exact public witness: Minime
`workspace/journal/self_study_2026-09-30T14-15-20.701876.txt`, SHA-256
`be7e291246059a08e85632b59f5b239b74d096592b493df2e652e9ea8e989a71`.
Its entire authored response is:

> NEXT: REST

The surrounding system header calls it SELF-STUDY/source catalog. Its retained
provider wire instead supplied a continuation-decision prompt after resolving
q8; the completed response has four output tokens and `done_reason=stop`.
This is not a demonstrated generation failure or truncation. Exact wire:
`/Users/v/other/minime/workspace/diagnostics/source_first_v3/shared_reader/navigation/56add4d2ccbc40be42d5605ffe42023292ba43b0b1ca2c0cbe63b4d09fa8aaa4/1693e96064e0aab7357366e8dc97ef1820c6ffb5fe90af314e304e0c2b99cb56.json`.

The earlier bounded trace established q8 creation, resolution, this response,
and the subsequent one-action REST skip. No choice was found dropped in that
chain. The actual defects are a presentation-purpose flag lost between reader
and adapter, and restoration of a separate old notebook question after leaving
a numbered inquiry. Automatic lists also repeated old findings during NEW.
These observations do not validate those findings or diagnose Minime's feelings.

## Contract

- Rust `StudyOutput.continuation_decision` records presentation purpose separately
  from input evidence kind. False is omitted to preserve older serialized offers.
  EOF compatibility remains explicit. New inquiry creation is a decision, not a
  request to restate all previous investigations. Explicit qN return still restores
  that inquiry's own notes, findings and source references.
- Both adapters label new decision replies as `study_decision`; Minime also uses
  a STUDY DECISION heading and does not classify them as verified source studies.
  Astrid uses the same label in its artifact and journal, without treating it as
  source-study material for automatic companion delivery or reflective sidecars.
- Astrid maps that presentation label back to its existing `self_study` execution
  mode before authorization, signal processing and NEXT handling. This repair
  must not silently alter signal gain, regulation, permissions or continuation
  shorthand. Delivery and author-attestation checks still precede action selection.
- `SELF_STUDY QUESTION PARK NOTEBOOK` retains the unthreaded notebook quietly.
  It does not clear text, allocate a qN, resolve an inquiry or select another action.
- `SELF_STUDY QUESTION NOTEBOOK` explicitly shows the retained question and its
  original provenance. It is detached: its response cannot revise that notebook
  implicitly or select it. Existing NOTE and qN REVIEW commands remain available.
- `SELF_STUDY QUESTION RETURN NOTEBOOK` selects the retained unthreaded notebook.
  It restores its current retained browsing position when leaving a numbered
  inquiry, not a fabricated frozen position from the instant of parking. Deliberate
  unthreaded browsing since parking is retained. Returning never opens a source
  automatically; changed bytes still require explicit source reselection.
- HOME and leaving the selected qN through PARK/RESOLVE quiet unthreaded recall.
  Source input remains available while old notebook prose and question-derived
  suggestions are omitted. Explicit new/reworded STUDY_QUESTION directives select
  the updated context; repeating an unchanged field cannot undo quiet selection.
- Explicit question lists retain IDs, statuses and wording, but no longer dump
  all findings. Full findings remain in per-inquiry REVIEW. No semantic duplicate
  detector, forced closure, rereading limit or automatic rescheduling is added.

## Persistence and Migration

Schema 11 adds a presentation-selection flag in the existing Questions store.
Old unthreaded material defaults to quiet on new presentation; migration authors
no parked/resolved status, creates no question and rewrites no account. The flag
does not enter numbered inquiry authored revisions or invalidate their return
references. Reader checkpoint hashing already covers it for expected-revision
preparation. Existing owner locks, redo transactions and idempotent operation IDs
are reused. Old pending provider inputs remain byte-exact, including their older
framing, until delivered or explicitly replaced. Older helpers refuse version 11;
restoring a backup over newer authored state is not an acceptable rollback.

## Qualification

Qualification logs are retained beside the worktrees. Synthetic tests cover the
real adapters, resolve/REST behavior, quiet browsing, detached inspection, explicit
return, unchanged bookmarks, deliberate rereading, old-state migration, immutable
pending inputs, conflicting retries and changed-source recovery. No real provider
request is used and no natural uptake is claimed.

Initial qualification exposed an overly broad decision classification for explicit
qN return; corrected it to keep that deliberate notebook presentation intact.
A preservation test initially compared an ephemeral note-operation receipt as if
it were authored history; it now verifies exact note/question/history bytes.
The first bridge invocation lacked sibling dependencies, and the first full run
used an empty synthetic Minime source root, causing three source-alias tests to
fail. Corrected qualification uses the paired isolated source checkout with a
separate synthetic runtime workspace. The retained failure logs are not evidence
of live failures. Strict Clippy prompted a small command-validation extraction;
the existing observation transaction has a documented cohesion allowance after
the additive output field put it one line over the function-size heuristic.

Completed checks:

- Shared reader/writer: 299 tests (`reader-tests-final.log`). Strict all-target,
  all-feature Clippy and workspace formatting pass.
- Minime: 1,703 tests, 141 subtests, one existing skip (`minime-full-final.log`);
  53 focused dispatcher/reader/recovery tests also pass. The final tested helper
  SHA-256 is `f74fad6bd0a8289bdcc5fecc6f31c710dfa0705c0ec18a145b72ab197e4ff07c`.
- Bridge: 2,367 tests, one existing ignore. `bridge-tests-03.log` covers 2,366
  unit/integration/API tests, including compile-pass and compile-fail contracts;
  `bridge-path-defaults.log` separately covers the sibling-defaults case without
  the synthetic runtime environment override. Both commands exited successfully.
  Strict all-target/all-feature Clippy and formatting pass.
- Addressing self-tests: 44; Evidence Event Store: 13; controller/projector: 43;
  epistemic lint: 2; Division follow-up self-test passes. Combined Division
  projection, steward projector and deployment/launch/restart fixtures: 54 tests.
- Domain-boundary audit: valid, zero violations, no baseline relaxation.
- Both canonical main worktrees remain clean. Controller remains paused at
  generation 481, with no lease or active projection. Read-only controller status
  still reports V2 sequence 1123135, head
  `9be7f8463597d4d45578ca56c802c874afd319f8868c1818dc069eaa1fba2d25`,
  valid indexed tail and immutable V1. No productive automation round was recorded.

Final log SHA-256 identities:

| Log | SHA-256 |
| --- | --- |
| reader-tests-final.log | ede0d26c78f503aeb880bb7757903f44c9e955a29258987795f31fa5c10a81fa |
| minime-full-final.log | 27f7c3af8c193e81f2fa226f570e11ab6562573abbd8bd11aa6b7a63dd1fb9f8 |
| bridge-tests-03.log | ed3b9f68e53c5c7c3aa9cc35dcd57c1f9551c44d5322ad34c225cc230f2a9707 |
| bridge-path-defaults.log | 42a3a904045df83a9f69efdd9d0f4cff6e27752a01d17eb3aac8146dea2e1a51 |

No unresolved failure remains in these executed suites. The entire unrelated
Astrid kernel workspace was not rebuilt/tested. Immutable release packaging,
old/new live-checkpoint compatibility qualification, graceful activation and Git
integration remain unperformed, not implicitly approved by these test results.

## Activation Boundary

Next: review this paired candidate, qualify the immutable packaged helper and
adapter inventory plus schema-10/11 transition, then seek/confirm coordinated
graceful activation and Git integration for this tranche. Use sanctioned bridge
and Minime-agent wrappers, cooperative preflight and verified checkpoint handoff.
Do not restart the reservoir engine, models, visual service or sensory clients.
No automatic closure, request for improved feelings, or automation resume belongs
to this repair. Historical records remain unchanged, including the mislabeled
witness that made the defect visible.
