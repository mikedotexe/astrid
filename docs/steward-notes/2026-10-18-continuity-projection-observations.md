# Minime Continuity: Reuse Diagnostic Inputs Within One Projection

Current status: qualified and gracefully activated agent-only on October 18,
2026; Git integration is recorded below when complete. Earlier qualification
and authorization boundaries remain as a chronological review trail.

## Status And Authority

October 18, 2026. Isolated implementation and synthetic qualification only.
Mike authorized tackling redundant continuity computation while preserving
freshness checks, transaction boundaries and durable receipts. This is not
authority to activate a new release or alter the concurrent MLX effort.

- Minime branch: `codex/continuity-projection-20261018`, based on local main
  `b0bedd98cd02a55a7a8a196a9614e810b1e9de03`.
- Minime worktree: `/Users/v/other/worktrees/continuity-projection-20261018/minime`.
- Astrid documentation worktree:
  `/Users/v/.codex/worktrees/continuity-projection-20261018/astrid`, from local
  main `0f486cff49e43436fbd8e23e07ab3624c0b9253a`.
- Artifact root: `/Users/v/other/worktrees/continuity-projection-20261018`.
- The two isolated roots have sibling symlinks for the existing paired test
  fixtures. Neither link points to a live checkout.
- No live-source edits, service operation, staging, commit, merge or push in
  this tranche. Existing MLX edits and larger-capacity work are excluded.
  No controller ownership or automation state is changed.

## Evidence And Motivation

The preceding survey fully read recent public writing and followed action/job
receipts. The public journal `workspace/journal/!journal_2026-10-17T13-53-31.510090.txt`
explicitly chose `WRITE FROM_JOURNAL`; content-free receipts then showed nine
completed `WRITE CONTINUE` jobs. The absence of subsequent public journal files
was not, for that interval, absence of work. This repair uses no private prose.

That journal's SHA-256 is
`c0a8c485a27eeee05baf30572b6b5b661a0211cb8c36a41889773116b37aa98a`.
The post-restart public navigation file
`workspace/journal/study_navigation_2026-10-18T13-55-59.622408.txt`, SHA-256
`6bf84dff9be753fc74bb649cb457c6373f5bbc1bf313f14aff31c124909202e0`,
selected `SELF_STUDY RELATE verify_read_only_research`.

Its completed job,
`job_minime_1792356811940_self-study-map`, has a complete phase-timing receipt:

| Measurement | Value |
| --- | ---: |
| Total worker interval | 272.893 s |
| Provider attempt, exclusive | 125.417 s |
| Continuity projections, exclusive | 108.293 s / 30 calls |
| Source fingerprints, exclusive | 29.054 s / 18 calls |

The receipt is Minime's
`workspace/llm_jobs/jobs/job_minime_1792356811940_self-study-map/phase_timings.json`,
SHA-256 `3aba1ca8c34fb1e04f9478bdc033830b118896a876ca7874fe86577ef6ca56f8`.
Exclusive timings avoid counting parent spans twice. Two preceding public
journal jobs also spent 86-90 seconds in projections/fingerprints versus 22-39
seconds in their provider attempts. These observations motivate reducing
repeated work, not a claim about subjective delay or a model defect.

## Implementation Contract

All production changes are in Minime's `minime_autonomy/runtime.py`.
The large pre-existing runtime remains in place; the small changes follow its
existing helper boundaries instead of mixing in a runtime extraction/refactor.

1. `_thread_projection` obtains the existing 200-distinct-event display window
   once. Its 8/32/96-event views, experiment display cues and proposal diagnostics
   use slices of that observation. Ordering, duplicate selection and limits
   remain unchanged. Raw event history used for stale-action reconciliation
   remains a separate read; it is not replaced by deduplicated history.
2. Cadence and stale-running diagnostics share one freshly constructed
   terminal-job map. A completion arriving after this observation is reflected
   on the next projection, rather than mixing two job states within those two
   diagnostic views. The job map is not retained on the store or in a global
   cache. Explicit empty inputs do not accidentally trigger another scan.
3. Standalone helper callers still read fresh inputs when no observation is
   supplied. Each later projection obtains new observations, including after
   append, replacement, rewrite, deletion, corruption or a previous failure.
4. Journal fingerprint enumeration uses `os.scandir` and each new directory
   entry's metadata rather than repeating `Path` metadata lookups. Every
   fingerprint invocation still enumerates the directory. Existing filename,
   symlink, file-kind and aggregate fingerprint semantics remain unchanged.

No full projection is memoized. There is no TTL, extra persistence format,
schema migration, changed authority policy or modified state/receipt writer.
The same thread JSON, NEXT document, event/observation records and SQLite
mirrors are written in the same order. Existing locks and worker outcome
capture are untouched. This does not claim an atomic snapshot of every file
in the system; it narrowly shares the diagnostic inputs identified above.

The qualification fixture retains 21 full projections and 12 fingerprint
calls. This first repair removes duplicate input work within those passes,
not every repeated projection across persistence boundaries. A broader change
would need its own dependency/transaction analysis.

## Synthetic Qualification

Only synthetic history is used. The test suite's filesystem/network guard
rejects live-state writes, live database access and runtime socket access.
The unchanged source-study helper is used for existing adapter tests:
`/Users/v/other/worktrees/truthful-return-20261016/bridge-stage-01/helpers/astrid-source-study`.

Focused coverage includes empty inputs, independent subsequent reads,
same-mtime replacement/rewrite, external-process modifications, late completion
between diagnostic views, corrupt and deleted job records, a failed read,
200-event duplicate ordering, unmodified caller input and journal scan parity.
Existing deadline tests retain timeout-versus-still-running distinctions.

The parent and candidate run the same synthetic `finish_action` fixture.
`records-before.json` and `records-after.json` compare equal byte-for-byte after
the existing test normalization of fixture paths and filesystem identity.
This covers all action-thread files and the thread/event/observation/artifact
database mirrors, not merely the final displayed summary.

### Timing And Counts

Two sequential, alternating parent/candidate trials use Python 3.14.3,
2,000 synthetic event rows (4,096 padding bytes each), 2,000 synthetic jobs and
2,000 synthetic journal files. Each trial has its own temporary state.

| Measure | Parent | Candidate |
| --- | ---: | ---: |
| Profiled finish, trial 1 | 4.190588 s | 3.225684 s |
| Profiled finish, trial 2 | 4.185794 s | 3.038545 s |
| Full projection calls | 21 | 21 |
| Terminal-job map constructions | 42 | 21 |
| Job-catalog snapshots, including freshness | 54 | 33 |

The mean measured interval falls approximately 25%. This is a small synthetic
CPU/filesystem comparison, not a forecast of production wall-clock savings.
Machine load and profiling overhead affect timings; exact scan counts are the
stronger regression target. No model call or live reservoir handle is involved.

Source identities embedded in the final benchmark receipts:

- Parent runtime: `22ba4371081d989a74bbaaab7aade3f221086fdd068d99b5e20e1353aa19974a`.
- Candidate runtime: `638ad58fca7d3b355757d7931b00979afa297b44eddd42acb99e27bf082fd4ff`.

Artifacts: `benchmark-before-1.json`, `benchmark-after-1.json`,
`benchmark-before-2.json`, `benchmark-after-2.json`, `records-before.json`,
`records-after.json`. Full-suite and deployment fixture outcomes are recorded
in the completion section below.

### Retained Unsuccessful Attempts

- Initial focused run: 285 passed, one assertion failed because a diagnostic
  mock still expected the old call signature. The assertion now verifies the
  explicitly supplied empty observation.
- New tests initially used an incorrect existing cadence-field name: 21 passed,
  one failed. Corrected the test to the actual contract field.
- Two baseline benchmark launches could not import pytest: setting repository
  `PYTHONPATH` shadowed Homebrew's `sitecustomize` path setup. Qualification now
  pins the interpreter and its site-packages path; no package was installed.
- Preliminary benchmark metadata accidentally hashed the public import facade,
  whose `__file__` intentionally aliases the runtime. Those artifacts remain
  as preliminary attempts. Final `benchmark-*` files hash the actual function's
  implementation source path.
- First complete Python run: 2,309 passed, two failed, two skipped, 167 subtests.
  Failures were missing paired-checkout fixture paths, not behavior mismatches.
  The initial deployment fixture run likewise lacked its sibling Minime path
  (121 passed, 14 failures including subtests). Isolated sibling links fix the
  test layout; no live path or test assertion was substituted.

## Completion And Remaining Release Work

Final-source results:

- Complete Minime Python suite: **2,313 passed, one existing skip, 167 subtests**
  in 120.89 seconds. Includes reader/writer, action admission, privacy,
  architecture boundaries, source-status, job lifecycle and new input-sharing
  regressions.
- Astrid deployment, handoff, launch-inventory, controller and Evidence Event
  Store fixtures: **129 passed, 40 subtests** in 17.60 seconds.
- Both worktrees pass `git diff --check`. No executable bytes changed after
  these final tests; subsequent edits only finish this documentation.
- Canonical Minime remains clean. At the final read-only check (15:19:39 PDT),
  PID 40182 still reports parent runtime SHA-256 `22ba4371081d989a74bbaaab7aade3f221086fdd068d99b5e20e1353aa19974a`,
  `source_changed_since_start=false` and `reload_required=false`. This is
  non-activation evidence, not a performance claim about the running agent.

Receipt SHA-256 identities (relative to the artifact root):

| Artifact | SHA-256 |
| --- | --- |
| `minime-suite-final.xml` | `f5629183a6f8c8dbd7e12dea1e8be49bb81d77e7766bb8e8b07a288cd96ae289` |
| `deployment-tests-final.xml` | `42e487890aa00bcddc9ec337262587c744325663317fdf4d09cf24b045f8d49e` |
| `records-before.json` and `records-after.json` | `6193301f67f6eea4c8d081b661b5f944168ccfee31bb07cf15978190b915d400` |
| `benchmark-before-1.json` | `0aee681f3aa05bca7916ac00dec3fa389298a3bfa6da0800c407b0a1c524d7ba` |
| `benchmark-after-1.json` | `4e7dee77092300a29cd83d531e4ce1f0d59ae3b8a42b61c13fd9774702341b9d` |
| `benchmark-before-2.json` | `37ba7d4c455c8471c556bdb835608d7adb8fe7986580a7d98155e829021f7065` |
| `benchmark-after-2.json` | `6eeba01883a06bae11a29cda28420e99a287aa1b256b020e2544808d473cc489` |

Test source hashes: `tests/test_action_continuity_performance.py` is
`7c2e0798514721fc555109c83c5d22019918239c2731ef0351d2d87c2de9fafc`;
`tests/test_continuity_projection_inputs.py` is
`75c8f5236269c7d9347800866875b99ae56e4153c9b790f250f28d09fdbe1b1b`.

Reproduction commands, from the respective isolated roots:

```sh
env PYTHONDONTWRITEBYTECODE=1 \
  ASTRID_SOURCE_STUDY_BIN=/Users/v/other/worktrees/truthful-return-20261016/bridge-stage-01/helpers/astrid-source-study \
  python3 -B -m pytest -q tests

env PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=scripts \
  /opt/homebrew/opt/python@3.14/bin/python3.14 -B -m pytest -q \
  scripts/test_reconcile_minime_launch.py scripts/test_restart_minime_agent.py \
  scripts/test_finish_minime_work.py scripts/test_minime_next_handoff.py \
  scripts/test_launch_inventory_contract.py scripts/test_paired_minime_handoff.py \
  scripts/test_steward_control.py scripts/test_evidence_event_store.py
```

For the profiled comparison, run the candidate's
`tests/test_action_continuity_performance.py::test_profile_finish_action_fixture`
with `--import-mode=importlib`, `MINIME_CONTINUITY_PROFILE_METADATA_ROWS=2000`
and a distinct `MINIME_CONTINUITY_PROFILE_OUT` path. Pin Python as above and
set `PYTHONPATH=<selected-source-root>:/opt/homebrew/lib/python3.14/site-packages`.
The parent root is the clean previously owned
`/Users/v/other/worktrees/minime-regulation-actions-20261017/minime` checkout;
the candidate root is listed above. Verify the embedded implementation hashes.

No Rust code or helper changed, so no Cargo build, Clippy run, substantial
build-cache allocation or cache cleanup was required. Python bytecode writes
are disabled for qualification. No authored/runtime evidence is removed.

Source-level qualification is complete; immutable release qualification and
activation have not been performed. Before activation: freeze and qualify the Minime-agent-only release against
fresh loaded identities, preserve current accepted work and pending NEXT,
coordinate with the MLX owner, and obtain fresh rollout approval. Use the
sanctioned graceful wrapper; stop on drift, failed drain or readiness failure.
No bridge/helper, provider, engine or sensory restart is part of this candidate.
No live-latency improvement, longer writing or subjective benefit is asserted.

Owned candidate paths:

- Minime: `minime_autonomy/runtime.py`,
  `tests/test_action_continuity_performance.py`,
  `tests/test_continuity_projection_inputs.py`, `CHANGELOG.md`.
- Astrid: `CHANGELOG.md`,
  `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`, this note.

## Immutable Agent Release Qualification

Completed October 18, 2026, after Mike requested release qualification followed
by fresh rollout approval. All artifact paths below are relative to
`/Users/v/other/worktrees/continuity-projection-20261018`.

`minime-reconciliation-01/source` freezes 101 current launch inputs and the
unchanged registry seed with read-only files/directories. Exactly one input,
`minime_autonomy/runtime.py`, differs from canonical and loaded Minime.
The previously deployed regulation-request repair is retained. This is an
offline, hash-bound source candidate, not a relocatable installation or a
separately frozen third-party Python environment.

### Results

- Frozen-source qualification: **616 tests and 37 subtests passed** in 60.46
  seconds. Covers projection inputs, continuity/history, job outcomes/timing,
  regulation admission, source status, pending NEXT and finish-accepted-work.
  Before/after import-origin checks bind 54/57 loaded autonomy modules to the
  exact frozen inventory; the projection implementation filename is checked.
  Subprocess crash/recovery tests inherit that frozen source path.
- Separate old/new processes preserve exact synthetic pending WRITE and
  regulation-request choices, selection IDs and envelopes in both directions.
  State and pending hashes match across each seal/restore pair; normal restore
  acknowledgements append. No generation is requested. This is compatibility
  evidence, not authorization to downgrade or restore historical state.
- Unchanged-helper preservation: **171 checks passed** using the currently
  installed immutable stage as both old and new. Exact drafts, pending inputs,
  receipts, reader/writer preparation, inspection and bridge-checkpoint fixtures
  remain compatible. No shared-helper replacement or schema migration is needed.
- Deployment/controller/Evidence Event Store fixtures: **129 tests and
  40 subtests passed**, 16.38 seconds. The earlier complete source-level
  2,313-test qualification remains bound to unchanged implementation bytes;
  it is not represented as a complete frozen-source suite.
- Current research/mailbox/handoff compatibility floors accept the candidate.
  Both candidate and canonical inventories still match reconciliation.
  Snapshot file modes, registry asset, helper stage and all loaded origin
  hashes verify again at closeout.

Retained unsuccessful attempt: the first frozen run had 615 passing tests and
one failure (37 subtests passed). An abrupt-worker test launched a child Python
process that could not import Minime: the driver set only its parent process's
`sys.path`. The revised driver also pins child `PYTHONPATH` to the frozen
snapshot. No production or test expectation was changed. Original script,
failure receipt, stdout/stderr and XML remain under
`qualify-frozen-agent-attempt01.py` and `frozen-agent-qualification-01`.
The successful rerun is `frozen-agent-qualification-02`.

### Read-Only Closeout

`release-review-01.json` records final identities at
`2026-10-19T00:02:34.306895+00:00`. Minime PID 40182,
boot `b43bff0a973e4904b58a2eaec63f91a7`, still loads the parent
runtime, with no reload required or source drift. Its observation was idle;
this is not an idle-window reservation or deployment readiness proof.
All ten protected service PID/start identities were readable and unchanged
between the two qualification closeout observations. The bridge/helper
selection is unchanged. No service signal, live-source write, authored-state
write, staging, commit, merge or push occurred.

Controller inspection found pause generation 530 still owned by
`codex-mlx-upgrade`, no active lease/projection, valid indexed-tail evidence
and immutable V1. The MLX task remains active; no message, ownership takeover
or shared service operation was attempted. Paused automations stay paused.
No Rust build, dependency installation, model call or cache deletion was
needed; free space at closeout was 429 GiB.

| Artifact | SHA-256 |
| --- | --- |
| `frozen-agent-qualification-01/failure.json` | `271e100b1fc54fee662fba82085c2f28859735e7d976b3435d1a86c8c34aa552` |
| `frozen-agent-qualification-01/tests.xml` | `2e0b8a29b4c8092688ca4ac08695c66b34c696a03a8eb06a050a6578d33eee59` |
| `frozen-agent-qualification-02/qualification.json` | `04c18e9754d83a4b8d04bc353d23efa7d7d22e3cbdaf50ed4f8d2a626c2d35e4` |
| `frozen-agent-qualification-02/tests.xml` | `2098694cc1031484fe527bcde1d6e707fff45a1e31a90aa1efd7a6d575ff92a1` |
| `minime-reconciliation-01/reconciliation.json` | `d53fb6c44993df465ab765fdab9864fd3716ec1cd35ae0dc9ccfb9280bab99d3` |
| `preservation-qualification-01/qualification.json` | `4fa5ed9afd8c3cbfe513b43143a7fb785b71c66b09086cc2f3268a8d2dd6acf7` |
| `qualify-frozen-agent-attempt01.py` | `06b4807413ae432e7fee069d86304ed07cc14c384c43be94126559fdc3f1e2e3` |
| `qualify-frozen-agent.py` | `ecea79b000ac716fcad8d7f9a86c24ff9e2eb38f46ed02bc1071c7e2aa3e357b` |
| `release-compatibility-01.xml` | `c6417a9088d447dbfc7255c29958f979658eae9f604c748726c350c1b4ab2818` |

Reproduce from the artifact root with the pinned Python interpreter:

```sh
/opt/homebrew/opt/python@3.14/bin/python3.14 -B qualify-frozen-agent.py
```

The driver deliberately refuses an already-existing output directory. For a
new run, use a new explicit output suffix and retain the previous script/hash
and receipts. Preservation uses the existing
`scripts/qualify_writing_admission_release.py`, the reconciliation above,
the unchanged truthful-return stage for both stage arguments, and the earlier
qualified `writing-research` binary. Its exact qualifier, binary, source and
stage hashes are recorded in `preservation-qualification-01/qualification.json`.

### Remaining Activation Boundary

Immutable qualification is complete. Fresh approval and an exclusive window
coordinated with the MLX owner remain required. Recheck launch/source identities
and cooperative preflight immediately before using the sanctioned
finish-accepted-work and `paired_minime_handoff.py --agent-only` procedure.
Do not run the paired bridge activation path. Preserve accepted output and
pending NEXT; stop on drift, failed drain, incompatible state or readiness
failure. Do not force termination or restore an older authored checkpoint.

After activation, verify fresh process/boot identity, all 101 loaded source
hashes, exact NEXT handoff, helper selection, and protected services. Live
latency/scan behavior must be assessed from naturally occurring jobs; synthetic
speedup does not establish a production effect. Larger capacity, providers,
engine, models, visual/sensory services and paused automations remain separate.

## Approved Agent-Only Activation

Mike approved coordination, graceful agent-only rollout and local integration.
The MLX coordinator recorded a live-service/shared-Git hold, including in its
own follow-up instructions. Both remote main tips matched the original local
bases. Controller ownership advanced from paused generation 530 to paused
generation 531, actor `codex-astra-interactive`. No automation was resumed.
Preflight verified the installed launch/source bindings, and all 1,660 foreign
Astrid paths were inventoried by content and mode before any installation.

The sanctioned `paired_minime_handoff.py --agent-only
--install-reviewed-overlay` installed the exact reviewed source snapshot,
preserved original source bytes and enforced its 185-second stability wait.
Exactly one launch input differs in content; all other installed paths remain
identical. Its first idle attempt refused at the pre-signal boundary: checks
took 7.937 seconds, moving source-status age from 50.089 to 58.025 seconds,
outside the 60-second interval's guarded idle window. No signal was sent.
The original receipt and owned launch hold were preserved.

The already-qualified `finish_minime_work.py` recovery consumed that exact
unsignaled receipt, revalidated current identities and used the running
`agent_drain_v2` protocol. SIGTERM was sent only to PID 40182. A study admitted
around the signal boundary completed normally; the wrapper verified its
durable outcome, zero active jobs, the acknowledged drain and native NEXT seal
before releasing replacement admission. No force, cancellation, blind retry,
state rollback, idle-check weakening or wrapper code change occurred.

Terminal success: `2026-10-19T00:16:23.778657Z` (October 18, 17:16 PDT).

- Old PID 40182 / boot `b43bff0a973e4904b58a2eaec63f91a7` replaced by PID
  5773 / boot `1e23c92c2e0540ababa2a394b22bbd9c`.
- OS launch started at 17:15:58 PDT while admission was held; native
  initialization occurred at 17:16:23 PDT. These are distinct timestamps.
- All 101 loaded inputs match the frozen candidate. Runtime SHA-256 is
  `638ad58fca7d3b355757d7931b00979afa297b44eddd42acb99e27bf082fd4ff`.
- Pending NEXT was present and preserved. Seal
  `c961fa8359f078043cad6af6a2894b0d565604168264ed50c8f134b4685ab690`
  matches restored acknowledgement
  `e2687bac7cc5857d13c560966e3c5799a2580175517545ce4b51c6d62545ef06`.
  This proves restoration, not subsequent execution or subjective continuity.
- The launch hold is absent; exact configuration/helper identity, all ten
  protected process PID/start pairs and 1,660 foreign files match the before
  receipt. Readiness passed; no loaded-source drift or reload requirement.
- A bounded read-only telemetry check observed advancing `t_ms` values
  1490249335, 1490251707 and 1490254073 with fill ratios 0.7102276, 0.7301039
  and 0.71025896. No engine restart or controller-settling claim is involved.
- Focused pre-integration rerun: 73 tests passed. No implementation bytes
  changed after frozen qualification. A synthetic Git fixture verified an
  explicit target-index fast-forward while preserving separate foreign edits
  and shared-changelog additions before canonical integration.

| Artifact | SHA-256 |
| --- | --- |
| `agent-rollout-01.jsonl` (unsignaled refusal) | `c04ef312141ffbd05e2c9e7d75a6586cd2e7b27ca484045767aed6e87c143c34` |
| `agent-rollout-02.jsonl` (successful acknowledged drain) | `0b17c88b996be9c4a11e762ce7f926c0ba41530296001c63fc84ed3ff966cb01` |
| `rollout-before.json` | `18cc9af7c7439142feb6141fec8b86fd9b299edc151cdb3d17fa1b40f2fcca38` |
| `rollout-after.json` | `ad36012dd1059c6c3de2dd014d5d891e9c5bab517aec431698ef0e3966d44b60` |
| `release-review-01.json` | `5c3378b4f5329ad420478276fac357b2632439b89a3b482180118fafbe24192d` |
| `preintegration-focused.xml` | `b86b8947c6b6a0abe3a36adc0b0bea72a9e77ec3fac893bce43c77078444db5b` |
| `qualify-git-integration.py` | `8ba18189d7fa1322a584f42309df7d4fb5a324d3f20b9c924711b8a4b2aeb273` |

Deployment succeeds independently of any later latency or authored-output
observation. MLX, larger-capacity work, providers, engine, bridge/helper,
visual/sensory services and paused automations remain unchanged by this work.

## Local Git Integration

Minime main fast-forwarded to
`58aa8c8e149665c8971abf86272182dd80a4f661` after full cached-diff review and
73 passing staged-state repair tests. The exact four committed paths are
`CHANGELOG.md`, `minime_autonomy/runtime.py`,
`tests/test_action_continuity_performance.py` and
`tests/test_continuity_projection_inputs.py`. The installed runtime was already
byte-identical; Git integration does not restart or rewrite the running agent.
Minime's worktree and index are clean. The commit records Codex provenance
explicitly rather than treating the shared Git author as agent attribution.

The Astrid documentation tranche contains only `CHANGELOG.md`,
`docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` and this note.
Canonical integration uses the exact reviewed target blobs for these paths,
retaining the separate uncommitted MLX changelog additions. It neither stages
foreign paths nor uses stash/reset/history rewriting. The synthetic integration
fixture above verifies this target-index fast-forward procedure.

Final post-integration identities, both commit SHAs, clean-index checks,
loaded-versus-committed input hashes and foreign-file preservation belong in
`rollout-after-integration.json`. For the shared changelog only, preservation
removes our exact added block before comparing the original foreign hash;
the combined file is not described as byte-identical to its earlier whole.
Return controller ownership to the MLX coordinator while still paused after
these checks. No push is included in this approval.
