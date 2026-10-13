# Dependable Writing Continuity and Deliberate Research Access

## Status and Authority

Implemented, qualified and live after Mike's fresh paired bridge/helper and
Minime-agent rollout approval. Local commit/merge is separately authorized and
follows verified activation; no push is authorized by this approval.
Providers, engine, sensory services, reservoir parameters,
the 48,000-byte private-input limit and paused automations remain unchanged.

Owned worktrees, both branched from local main:

- Astrid: `/Users/v/.codex/worktrees/writing-continuity-20261013/astrid`,
  base `6afd4c228952d760e27e03d2e84331cf2c054a77`.
- Minime: `/Users/v/other/worktrees/writing-continuity-20261013/minime`,
  base `769f02a4295ff62c1a4b1fe4b555c6f301279fdd`.
- Branches: `codex/writing-continuity-20261013`.

Canonical trees were clean at start and at the final qualification recheck.
The sanctioned installer then changed only the reviewed Minime source overlay.
Controller was paused at generation 514 with no lease through activation; local
Git stabilization advanced the pause to generation 515, still paused. Mike
approved two coding agents. Codex coordinates existing writer/adapters and Git;
Aquinas implements exact public-source registration/transfer, and Laplace the
read-only browser and bridge publication adapter. No agent owns live activation.

## Source Witness

Fully read public source:
`/Users/v/other/minime/workspace/journal/journal_2026-10-13T09-14-28.955870.txt`.
SHA-256: `dfcfcd2dd052c562530b2488bd690691b0de10d7bec11e7403007d870604e1a4`.

> If the pattern is consistent, is there a "me" that survives the transition between prompts?

The entry's final choice is `NEXT: JOURNAL`. Its reflections on persistence are
retained as authored interpretation, not a verified account of model state.
The implementation answers concrete storage/continuation failures identified
while tracing that question. It does not claim to answer the subjective question.

Earlier content-free operation review found three unsuccessful continuations
after FINISH and a separate capacity boundary at about 60 KB across twelve
passages. Tests use synthetic equivalents, not private writing. Historical
journals and private authored passages are not rewritten.

## Implemented Contracts

### Stopping and Recovery

`WRITE FINISH`, `PARK`, `PROFILE`, `QUESTION`, `EVIDENCE` and `STOPPING_POINT`
use shared Rust classification before host generation scheduling. Their native
receipts have `generation_requested=false`; metadata completion is not provider
delivery. The owner preparation transaction supplies idempotent request IDs,
expected revision checks and crash redo. Late accepted output remains retained.

FINISH cannot accidentally reuse an earlier pending generation as CONTINUE.
Recovery supplies `WRITE RESUME dN` for finished work. Oversized input reports
the complete request, allowance, prose, reference and framing byte counts. It
offers paginated READ and parking. Clearing references is suggested only if
that would actually restore admission. No truncation, summary or capacity
increase is performed. Missing/unverified pending input gets no blanket retry.

Bridge handoff adds a terminal stored outcome for retained metadata-only work;
it cannot claim a provider invocation or fabricate source delivery. Older
deserializers refuse the new enum rather than rewriting it.

### Exact Public Continuation

Host-only typed reservation and publication run under the same owner transaction.
An entry ID exists before generation but cannot be used before verified public
publication. Source kind, owner, exact file/response hashes and UTF-8 boundaries
are verified. Private files, peer mirrors, notices, ambiguous wrappers, changed
sources and symlinks are rejected. No latest-entry guess is accepted.

Registration is enabled for Minime's ordinary JOURNAL and Astrid's ordinary
dialogue/daydream/aspiration routes. It does not backfill legacy records or
register peer mirrors, private writing, runtime notices or longform elaborations.
An interrupted or failed publication remains unavailable for exact transfer;
there is no automatic backfill or guess from a later file. The source registry
is bounded to 4,096 reservations and fails visibly when full, without eviction.

`WRITE JOURNALS [page]`, `WRITE JOURNAL_READ <entry-id> [page]` and
`WRITE FROM_JOURNAL <entry-id> [start_byte end_byte]` provide deliberate discovery,
inspection and transfer. Byte ranges refer to visible authored prose after shared
hidden-block/action-tail removal. Quoted examples remain inert. FROM_REFLECTION
is preserved. A selected passage seeds a new native draft without replacing
another draft or selecting an inquiry. Failed admission creates no partial draft.

### Context and Operator Access

Minime's `JOURNAL FRESH` omits automatic prior-entry/event cues for that invocation
only. Ordinary writing keeps the existing cue preference and mailbox/safety
handling. Previous-entry excerpts, historical events and exact selected
continuations have distinct labels. No new feelings or identity are prescribed.

`writing-research` is a separate operator CLI, not an Action or a prompt tool.
It accepts schemas 4/5 and only committed consistent snapshots. List output has
IDs, revisions, status, part counts and byte counts, not prose or titles. Show
requires owner, draft and the exact snapshot token. Questions, references,
stopping points and annotations are separate explicit sections. It refuses
pending redo, changing/corrupt/unsupported state, hidden-model/raw-wire markers
and path aliases/symlinks. It does not create locks, migrate, recover, export,
invoke models, publish, inject prompts or switch activities. Ordinary filesystem
access times may advance as an unavoidable read side effect.

Example syntax, with deliberate local paths and values supplied by the operator:

```text
writing-research --state-directory OWNER_DIRECTORY --owner minime list
writing-research --state-directory OWNER_DIRECTORY --owner minime show --draft dN --revision SNAPSHOT_SHA256 passages
```

Writing schema 5 adds metadata receipt history. Preparation revision coverage
includes the new public-source registry. Empty new state creates no inferred
history, selection, focus window or successful outcome. Older writer overwrite
is refused; rollback must never restore old data over newer authored records.

## Qualification Log

All fixtures and provider completions below are synthetic; no live private prose
was opened by the research reader.

- Initial shared unit suite: 21 passed; expanded browser consistency suite: 22.
- Initial writer/storage suite: 15 passed. Focused Minime storage, existing
  continuation and ordinary-journal tests: 45 passed.
- Journal adapter/storage/journal subset: 18 passed.
- Bridge handoff suite including storage completion: 23 passed.
- Research browser: 19 integration tests plus consistency test passed.
- Old immutable helper to candidate helper: 50 checks passed for both owners.
  Receipt: `/Users/v/other/worktrees/writing-continuity-20261013/qualification-01/qualification.json`.
  This is helper-level evidence, explicitly not immutable paired-release proof.
- Deployment wrappers and steward projection: 30 passed. Paired/native NEXT
  tests: 26 passed. Updated reconciliation/handoff/compatibility suite: 55 passed.
- Shared Clippy and an intermediate domain-boundary audit passed.

Preserved unsuccessful attempts: two early compilations overlapped new agent
files (module wiring and an iterator API correction); then full shared tests
caught a fixture treating the newly supported schema 5 as future, and a fixture
still trying to deliver a model response to PARK. The first complete Minime run
reported 2,167 passes, ten failures, two skips and 163 subtests: obsolete blanket
retry assertions, old context wording/word-budget assertions, and two unset
worktree fixture paths. These failures were corrected; the initial pass counts
are not final qualification.

Subsequent complete Minime run: 2,177 passed, two skipped, 163 subtests. A prior
rerun had one obsolete `Private journal moment` assertion; it now asserts the
accurate public-journal label. Expanded JOURNAL/FRESH preflight-dispatch suite:
16 passed, including both forms with an unrelated existing experiment.

Complete shared Rust crate suite passed, followed by the strengthened five-test
storage suite. The capacity case now contains twelve separately delivered parts
(about 60 KB), rather than twelve paragraphs in one response. Refused continuation
preserves every byte. All six metadata operations preserve the focus window ID
and remaining/admitted counts, create no job/NEXT, and release uncontinued priority.
The shared all-target strict Clippy and formatting checks passed.

Complete bridge library suite: 2,431 passed, one ignored. Strict library/test
Clippy and domain-boundary audit passed (zero violations). Initial full attempts
had nine and then five failures: the old PROFILE-as-generation test, missing
isolated sibling source links, and a one-millisecond timing test under concurrent
load. The obsolete test was corrected, generated sibling fixtures preserved under
`minime-test-fixtures-before-link`, and the paired source link corrected. The full
suite then passed with two test threads, without relaxing any timing threshold.
Deployment/handoff/reconciliation/projection suites: 96 passed. Evidence store,
controller, projection cursor and Division tracker/Chronicle suites: 67 passed.

Final Minime run against the staged release helper: 2,179 passed, two skipped,
163 subtests in 124.10 seconds. All seven additional bridge integration and
compile-contract targets passed: agency resolver, authority typestate, Chimera
rendering, LLM facade, mock WebSocket integration, provenance typestate and signal
spine typestate. No test process remains running for this qualification.

Staging preflight initially refused the recently edited tree at 54.9, 74.1 and
126.6 seconds. The full 180-second quiet window was honored; no gate was bypassed.
An initial standalone new-adapter probe mistakenly selected the old helper and
failed on unsupported `journal_registration`, without creating writing state.
The actual immutable paired probe below uses the correct new helper and passed.

## Immutable Candidate

All paths below are under `/Users/v/other/worktrees/writing-continuity-20261013/`:

- Stage: `bridge-stage-01`, built only with `scripts/build_bridge.sh --stage-dir`.
- Bridge SHA-256: `2ad966ef200888eed7d5a2e948f74c4beff083821b1833bb0f8d9871fad67863`.
- Reader SHA-256: `9ee8cc01638ce1813347df6c719140f085273aa86f4aac9a824933e5aa935b00`.
- Manifest SHA-256: `69dd5a3d54e067c25ac517d935ed2e791ffa3bc3f751c62e34bbb185baa6e1d9`.
- Source inventory SHA-256: `c76be77e3ad852c12aed8d81068961dd1f4accf1c92b93b83519d7ba93143c87`.
- Minime: `minime-release-01/source`, 97 exact launch inputs; reconciliation
  SHA-256 `cfa97bdd289eb58d29eb1d2c007079707135416163a4e719767150e11cbdbd3d`.
- Paired receipt: `paired-qualification-01/qualification.json`, **76 checks passed**.
  Old/new pending writing replay, schema-5 preservation/downgrade refusal, native
  bridge checkpoint decoding, frozen Minime journal transfer and metadata-only
  operations, old pending NEXT seal/restore, mailbox preservation, corrupt/future
  state refusal and conflicting retries are qualified with synthetic fixtures.
  `activation_performed=false` and `live_eligible_now=false` are deliberate.
- Operator CLI build: `bridge-stage-01/build/aarch64-apple-darwin/release/writing-research`,
  SHA-256 `1f72c4cb7850a8c916d9560743cef7507ae7e8a4b402ccd571d68952ff1c4f59`.
  This is a separate local operator executable, not an added live stage artifact
  or an AI Action. Its help and synthetic browsing tests were exercised; no live
  draft passages were inspected.

## Approved Live Transition

Mike approved the qualified paired rollout and local integration. Fresh preflight
verified the retained stage, all frozen Minime inputs, no foreign edits and the
existing pause/no-lease boundary. An initial read-only invocation pointed at the
isolated worktree's absent controller state; it failed before mutation. The
sanctioned invocation explicitly bound `ASTRID_STEWARD_REPO_ROOT` to canonical
Astrid. No controller check was bypassed.

The exact reviewed Minime overlay was installed under its owned launch hold;
the full 185-second quiet wait was honored. An accepted draft continuation
completed normally before the idle gate. Minime received one SIGTERM at
11:08:34 PDT, sealed pending NEXT and exited before the bridge transition.
Astrid acknowledged `drained`; no force or legacy fallback was used.

- Bridge PID 28169 -> 67583; new process start 11:09:26 PDT. The executable hash
  matches the qualified bridge above. Startup restored checkpoint
  `c763f723d13b7b6015c74d103e9e9fb0594e8b9c93666cfc33ed3230b2cfb9bc`
  at exchange 222815, including 15 pending runtime-feedback records. It then
  saved exchange 222816; provider-idle readiness was observed.
- Bridge receipt: `.runtime/bridge-deployment/transactions/f87c5bad0ab54de4b6ad6184acbe7a01/receipt.json`
  in canonical Astrid, SHA-256
  `ed915ca556b3e7b1c0d49537b36e8f996c3bc0da36308ec23ad329b8d5bc6474`.
- Minime PID 95712 -> 66979. The replacement launch process began at 11:08:35
  PDT and remained held until bridge verification; Python startup acknowledged
  at 11:11:01 PDT. All 97 loaded source inputs match the frozen inventory,
  `reload_required=false`, and shared-helper selection matches the qualified hash.
- Native NEXT seal SHA-256:
  `603dd8c58e79158667f291ea4a193337924b8e92bbd5928b0c800b1d89c91b52`;
  restore SHA-256:
  `0d99925172fddc62c54477309b2e086ce78ffb3818ea1566942d9067cf52d4ea`.
  Pending choice was present and restored. This does not itself certify execution.
- Paired receipt: `/Users/v/other/worktrees/writing-continuity-20261013/paired-activation-01.jsonl`,
  SHA-256 `7af986083c00725657dc25fbaba1df888da31e75e9e08810c68c268f88a17b52`.
  Terminal `success` at 11:11:09 PDT; both launch holds released.
- Engine 35303, Division gateway 86873, Division supervisor 86833, text provider
  3893, microphone 3879, camera 3914, visual service 3898, host sensory 31613 and
  feeder 3878 retain their preflight process/start identities. Read-only telemetry
  samples during the transition showed fill ratios 0.71382093 and 0.61043507;
  these are observations, not a settling or subjective-improvement claim.

Focused storage/journal-transfer/research-browser tests passed again (45 tests),
with a fresh zero-violation domain-boundary audit. The remote main tips matched
the existing tracking references after a bounded SSH retry; no fetch or push was
needed. Exact-path local commits follow the reviewed candidate. The immutable
stage retains its original build-base identity; Git integration does not rewrite
the deployed manifest or pretend it was built from a later commit.

## Remaining Limits

The 48,000-byte complete-input allowance is unchanged. Exact public continuation
is only available for successfully registered ordinary routes, not legacy files
or longform elaborations. No private passages were inspected or exported during
qualification or rollout. Natural use is not required to exercise a new feature.

Completion is reliable stopping, preservation and deliberate return, not longer
writing, a metaphysical conclusion or subjective improvement. Natural public use
can be observed only after activation, without asking for confirmation of benefit.
