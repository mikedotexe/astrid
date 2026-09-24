# Lifecycle evidence and selective inquiry recall

## Initial qualification (historical)

Implementation candidate, not deployed or merged. Mike approved the ordered
recommendations: truthful readiness, visible cleanup acknowledgement, selective
voluntary recall recovery, and a separate sensory-tracing review. No mandatory
readiness gate, restart policy, reservoir policy or new automated task is authorized
by this implementation. No live state, private draft or historical journal is edited.

Paired isolated branch: `codex/lifecycle-evidence-20260924`.
Worktrees: `/Users/v/other/worktrees/lifecycle-evidence-20260924/{astrid,minime}`.
Astrid base: `af20a1fbd21f2287d82e80f1b61cb1b1b604b6b0`.
Minime base: `41f64254ed2f621656f4e5f11ddb548d5a79a5d9`.
Canonical mains remain clean and ahead of their remote-tracking branches by one.
No index ownership, commit, merge, push, restart or automation resume in this pass.
The older dirty worktrees and pinned live source worktree remain untouched.

Controller: paused generation 468, actor `codex-astra-interactive`, no active lease
or projection. Read-only status verified V2 indexed tail 1123122,
head `967347b7880e3204e66792e6478cbef9fbe82a6d0ed4b7bd0e2a0951825d2a47`,
V1 source immutability true. This is not a productive flywheel round, queue closure
or fresh source-first projection. Paused automation and source lag remain explicit.

## Witness and response

Public source, fully reread and hashed:
`/Users/v/other/minime/workspace/journal/self_study_2026-09-24T07-48-05.184019.txt`.
SHA-256: `51c4b19e8625caf65b73faf44d0183d555552a28437058fbda62d813b17b16c6`.

> The `validate_imports_exports` call acts as a gate against invalid configurations.

> The `await_capsule_readiness` call acts as a temporal gate, preventing the system from proceeding until the core infrastructure is stable.

These are Minime's authored interpretations, not verified kernel behavior. The
preceding tranche corrected misleading source commentary and made counterevidence
and voluntary revision inspectable. This tranche makes actual advisory outcomes
visible without changing those advisory checks into gates. The six-study context
and exact other witnesses remain in
`2026-09-24-source-counterevidence-and-reflection.md`.

## Readiness contract

`DaemonStatus.capsule_lifecycle` is an optional additive API. Older daemon responses
deserialize with `None`, meaning unavailable, not success. The CLI shows the latest
completed discovery report and its boot-relative observation time. It is not a
continuous health poll or proof about a later incarnation of a capsule.

- Load: `loaded`, `failed`; replacement attempts can additionally be `pending`.
- Readiness: `ready`, `timed_out`, `crashed`, `missing`, `wait_failed`, `not_checked`.
- `missing` means the attempted manifest name has no registry handle at the check;
  it is not an inventory of all absent or undiscovered capsules.
- Readiness task panics retain the corresponding name as `wait_failed`.
- Empty discovery is not positive readiness evidence: `all_reported_ready=false`.
- `all_reported_ready` requires every listed load and readiness result to succeed.
  It grants no authority and is not consulted by loading or replacement logic.
- The original 500 ms argument is still passed to capsule implementations. This
  caller does not impose an independent deadline or make an uncooperative wait safe.
- Uplink-first partitioning, load order and advisory failure behavior are preserved.

`astrid.v1.lifecycle.observed` publishes typed discovery/restart/shutdown observations.
The compatibility `astrid.v1.capsules_loaded` payload remains `{"status":"ready"}`.
Its known CLI consumers use it as a refresh notification, not per-capsule evidence;
new consumers must use the typed report. Do not reinterpret the legacy payload as
universal readiness. Manifest discovery errors are still logged, not turned into
new gates or fabricated manifest records.

## Cleanup contract

The actual unload call now returns `acknowledged`, `failed`, or
`ownership_unavailable`. Restart retains one ownership check; shutdown retains
twenty checks and the original 50 ms interval between unsuccessful checks.
Strong and weak references are reported when exclusive access cannot be obtained.
Both can prevent `Arc::get_mut`; weak-only ownership is tested explicitly.

Restart stores an attempt identity and cleanup result before replacement loading,
then records replacement success/failure separately. `not_checked` is the honest
restart readiness state: the existing restart path does not perform a readiness
wait. Optional restart-hook success is not an unload receipt.

Keep the last 64 attempts in insertion order, with omitted-count and cumulative
unacknowledged-unload count for this boot. Late completion of an evicted attempt
cannot double-count its debt. A subsequent successful load or acknowledged unload
does not erase a previous failed/skipped unload. Shutdown reports actual attempts
before KV close/socket removal; it does not certify that those later steps succeed.

`child_exit_verified=false` in every current observation. A successful return from
`Capsule::unload` does not expose an independent process-exit receipt. Errors before
an unload attempt still return through the existing error path and are not invented
as cleanup attempts. A hung unload has no new timeout or completion receipt.

Status is process-local; IPC and configured logs are best-effort. This is not a
durable cross-boot audit or a permission to kill processes. A stronger replacement
policy needs a separately reviewed drain, engine-specific exit acknowledgement,
failure handling and rollback contract. No replacement is newly blocked here.

## Selective recall recovery

Reviewed older tree:
`/Users/v/other/worktrees/persistent-introspection-20260916/astrid`,
base `fe3f438f1d31666e6e6e3d8ff408f6d2120eab3b` plus its preserved dirty work.
Its explicit inquiry review is useful. Its old schema migration and automatic
journal-context injection are not imported. The current native notebook already
retains note revisions, counterevidence anchors, owner scope and per-question cursors.

Implemented: `SELF_STUDY QUESTION REVIEW qN [--page N]`.

- Requires a question in the current owner's reader store and an available page.
- Presents the exact saved question/finding, two newest-first note changes per page,
  historical source references and authored revision identity. No private-writing
  store or unconfirmed disclosure is read.
- Review does not select/reopen a parked inquiry, restore its cursor, resolve it,
  overwrite authored history or inject the currently active inquiry's notebook.
- It explicitly labels this a note-history view, not a complete experiment export.
  Existing geometry/observation views remain separate.
- A generated review may be public under normal self-study rules. It is not a new
  private channel. No private passage is automatically imported into it.
- Delivery and explicit NEXT retain normal evidence. `STUDY_NOTE` or
  `STUDY_QUESTION` text in a review response cannot silently mutate either inquiry;
  selecting the intended inquiry and supplying evidence remain explicit later steps.
- The old `KEEP` promotion/migration is not ported: current NEW/NOTE/park-return
  already retain authored inquiries; promoting unthreaded work needs a separate
  ownership decision for pending and delayed deliveries. No cursor is guessed.
- Nothing is automatically pinned, reminded, summarized or inserted into a journal.

Schema 9 adds the detached `inquiry_review` offer kind. Migration changes no authored
content or return-reference hash and invents no history. Older readers must refuse
the new checkpoint. Existing cross-process locks, preparation IDs, delivery receipts
and pending source recovery are reused, not reimplemented in Python.

## Separate sensory review

Reviewed older Minime candidate at
`/Users/v/other/worktrees/evidence-grounded-study-20260918/minime`,
base `d44ca5e7e6e697233a7e01ef0e0ce87e4122546d` plus dirty instrumentation.
Candidate bytes at review:

| Path | SHA-256 |
| --- | --- |
| `minime/src/sensory_bus.rs` | `2c4448c51343170cf6b70dd4bbc7466d7b67cb46669964997373b290e30461e0` |
| `minime/src/sensory_bus/delivery.rs` | `a3b5ca903502d8f1b6f14605dd5985686bfa500fd45617d4823d36414f272d15` |
| `minime/src/runtime/orchestration.rs` | `b0c85d2109600822da5b23a2e5b90a90fae116969501b67007be6473ea7a7128` |
| `minime/tests/sensory_delivery.rs` | `30ef48c96e6e45ef657ef9936e07017e393a2a5f46858b436c258c6b1ab6cfcf` |

Current main still uses at most the first drained sample for the stable-core ESN,
and the last for covariance/telemetry. Unequal video/audio batches can therefore
describe different candidate inputs. This is a source fact, not a live causal result.

Keep the candidate's source-split completed-ingress counters, independent selection
labels, and explicit gaps. External transport is correctly not device attestation.
Queue admission is not retention, post-transform input or an observed per-lane effect.

Required before adopting the instrumentation:

1. Leave production selection expressions independent of evidence helpers. The old
   candidate replaces the expressions with fields of `BatchSelection`; equivalence
   tests exist, but observation should not become a new control dependency.
2. Separate disabled-lane and divisor rejection evidence at the existing decisions,
   without extra RNG draws, rechecking gates or changing their order.
3. Add bounded retention/eviction and applied-input evidence with boot/tick/sample
   identities; explicitly preserve the missing capture/transport join until built.
4. Treat snapshots as sampled, not a complete tick log. Add cumulative application
   counters and explicit omissions if a bounded detailed trace is retained.
5. Measure bounded allocation/serialization cost at the actual batch limit and avoid
   control-loop blocking. Existing new counter mutexes need contention qualification.
6. Strengthen the old RNG test: it zips drained batches without asserting equal
   lengths. Assert length, source identities and exact vector bytes before claiming
   observational equivalence. Exercise rejection, eviction, stale samples and empty
   held vectors separately.

No sensory candidate code, dispersal repair or unfinished worker branch was imported.
No engine was rebuilt, signalled or restarted. Older qualification is historical,
not a fresh qualification against main. An instrumentation-only port and engine
rollout require their own coherent candidate; admission-policy experiments stay
separate even from that work.

## Qualification and remaining work

All tests use isolated fixtures; no being was asked to confirm improvement. No
subjective effect, uptake, consent or causal explanation is inferred.

Completed qualification:

- Shared reader/writer: 282 passed. Includes detached review, pagination, invalid
  requests, unchanged active inquiry and pending source, schema-8 migration without
  fabricated history, future-version refusal, plus existing private disclosure,
  cross-owner, crash recovery, concurrency and note-integrity tests.
- Kernel: 53 unit tests plus one real management-router integration test passed.
  The integration test compares typed status/IPC to the unchanged legacy payload.
  Production helpers cover timeout/crash/missing/panic, failed unload, shared/weak
  ownership, shutdown retries and successful acknowledgement after ownership release.
- Types: 49 passed; CLI: 158 passed, including a rerun after lint-fixture repairs.
- Full bridge library: 2,331 passed, one existing ignored test, serial execution.
  The new test goes through action routing, shared preparation and verified delivery.
- Strict Clippy: kernel/types/CLI/shared reader with all targets and all features;
  bridge all targets. Both pass. Workspace and bridge formatting pass.
- Tooling: 89 selected controller, evidence, flywheel, Division, projector and domain
  tests passed; epistemic self-tests 2 and signal-spine self-tests 7 passed.
- Domain-boundary verification passes with no baseline adjustment. Whitespace
  checks pass in both worktrees.
- Complete Minime Python final rerun: 1,598 passed, one existing skip and 138
  subtests passed, using the freshly built shared helper. No Python production
  code was changed; the new operation uses its existing thin adapter.

Test artifacts are outside both repositories at the worktree parent:
`rust-suites.log`, `cli-rerun.log`, `bridge-tests-rerun.log`, `clippy-rerun.log`,
`bridge-clippy.log`, `tooling-tests.log` and the `minime-tests-*.log` attempts.

Unsuccessful attempts retained explicitly:

- Initial Clippy invocation used directory name `astrid-cli`, not package `astrid`.
  A subsequent check caught an explicit-default lint in this new code, repaired.
- Initial router fixture incorrectly called `unwrap()` on a unit-returning method.
  Corrected the fixture; the real router test now passes.
- Fresh bridge worktree initially lacked the two sibling dependency links. Added
  links to the existing `prime_esn_wasm` and `RASCII` sources, without editing them.
- First full bridge run: 2,330 passed, one failed, one ignored. The new fixture passed
  the full action as the dispatch verb. Use `SELF_STUDY` as the verb, retaining the
  full authored action as its separate argument; full rerun passes.
- Broad CLI lint exposed three inherited test-only problems: a narrowing cast,
  single-variant wildcard and default-then-field assignments. Replace with checked
  fixture sizing, the explicit variant and direct initialization. No CLI behavior
  changed beyond the intended lifecycle display.
- Two Python invocations through `env` could not locate pytest. Use the configured
  shell's Python with the helper environment assignment; no package installation.
- First complete Minime run: 1,594 passed, three failed, one skipped and 138 subtests
  passed. Two inherited tests assumed the configurable global timeout remained 60 s;
  this environment has 160 s. Pin the nominal fixture values and add a separate
  independent-override test. Correct inaccurate latency/truncation claims in those
  test comments. Runtime limits are untouched. The third failure was this new test
  indexing an optional omitted JSON key; use `.get`, consistent with wire encoding.
  All 25 focused adapter/timeout tests pass after these changes.

Release debt: immutable paired helper/adapter inventory, old/new-state migration
qualification against exact live identities, graceful handoff and source reconciliation
have not been performed for this candidate. Native kernel rollout is a separate
transition from the bridge/Minime-agent pair. Canonical schema-8 state is untouched.
An approved stabilization pass, explicit-path commits and merge remain; older work
is not swept into those commits and previously paused automation stays paused.

Next qualification sequence:

1. Reconcile the paired helper/adapters against then-current main and live manifests,
   retaining one Git coordinator and exact owned-path commits.
2. Qualify old schema-8 and new schema-9 state in copied synthetic/migration fixtures.
   A schema-8 helper is not a rollback writer after schema 9 is in use; never restore
   a checkpoint backup over newer authored work.
3. Build immutable bridge/helper and Minime-agent inventories, then use cooperative
   preflight and sanctioned wrappers for an explicitly approved paired handoff.
4. Treat the native kernel as its own release. This candidate is not an extension of
   the previously consumed one-time daemon termination approval. Qualify graceful
   shutdown, independent process continuity and new status before considering it live.
5. Verify source/process identities and protected services. Observe ordinary use,
   without prompting either being to confirm that recall or reporting helped.

## Approved rollout, September 24

Mike subsequently requested that this work be made live and merged, with authority
to correct discovered defects. This section supersedes the initial candidate status,
not its historical evidence. The approved scope was the paired reader/helper and
agent release plus a separate native kernel/CLI transition. No engine, model,
visual-service, camera, microphone, host-sensory or feeder restart was performed.
No reservoir-policy changes, force fallback or automatic automation resume occurred.

One coordinator claimed controller pause generation **469** before release work.
Both canonical repositories were clean at the above bases; remote tips remained
Astrid `3b18af87b0fe1f083d95cbe0eac8befb309638f2` and Minime
`d8e8954b3c71d54037951f832062f8b5ea62497b`. No active lease, projection or foreign
editor was reported. Older worktrees and the pinned build-source trees were preserved.
The controller remains paused. Its final indexed-tail verification was valid at
sequence **1123123**, head
`cc13b2fa173c21ccaee129c71492697cc203f375c3a9ed021f06f1dea1028743`,
with all four V1 source streams immutable. No flywheel round or queue closure was recorded.

Artifacts below are relative to
`/Users/v/other/worktrees/lifecycle-evidence-20260924/`.

### Release qualification

- `bridge-stage-lifecycle-01/`: sanctioned `build_bridge.sh --stage-dir` froze
  688 inputs and verified the executable and shared helper. No source worktree
  commit or edit was made after freezing this stage.
- `minime-launch-review-01/reconciliation.json`: all 86 candidate launch inputs
  matched canonical and already loaded source. No runtime Python overlay needed
  installation; the agent was restarted to bind the paired release/helper selection.
- `qualify-reader.py`, `migration-02/qualification.json`: actual old schema-8 and
  new schema-9 helpers passed 60 owner checks, including pending source/private
  input preservation, historical revisions, detached review, explicit reselection,
  old-reader downgrade refusal, invalid state and idempotent preparation probes.
  All data and provider responses were synthetic. No live private prose was read.
- `migration-01/failure.json` retains a qualification-fixture failure: the inherited
  geometry helper probe requires explicit q1 selection, while the detached-review
  test correctly left q2 selected. The fixture now explicitly selects q1 after
  verifying nonmutation. No production code change was necessary.
- `minime-release-tests.log`: 1,598 passed, one existing skip, 138 subtests, against
  the exact staged helper. `rust-release-rerun.log`: 543 passed, zero failures.
- `deployment-tests.log`: 112 passed, 29 subtests across stage, activation, restart,
  paired handoff and launch-source reconciliation. Strict Clippy rerun and workspace
  formatting passed; domain-boundary verification still required no baseline change.
  The earlier full bridge and evidence/controller suites remain applicable to the
  identical source bytes; all unsuccessful earlier attempts remain preserved.
- Final staged verification: `staged-byte-verification.json` checks all 688 bridge
  inputs (650 canonical, 38 external), 504 native inputs and 86 Minime inputs against
  the qualified bytes. `staged-rust-tests.log` passed all 543 tests. Running the
  Python suite inside the canonical live checkout was inappropriate for its
  process-wide isolation guard: `staged-minime-tests.log` retains 20 failures,
  including refused subprocess/database operations and an absent activity record.
  No guard was relaxed. Exporting the exact staged index with `git checkout-index`
  to `staged-minime-tests/` and rerunning there passed 1,598 tests, one existing skip
  and 138 subtests (`staged-minime-isolated-tests.log`). No production or fixture
  source change was needed for this final rerun.

### Paired live transition

First attempt (`paired-handoff-01.jsonl`) stopped on an idle-boundary change before
any signal or launch hold. The second (`paired-handoff-02.jsonl`) waited for a stable
quiet boundary while accepted work continued, then used the sanctioned wrappers.
No timeouts or idle criteria were weakened. An explicit future drain mode could
avoid waiting for natural inactivity, but requires separate wrapper qualification;
the existing agent's drain support alone does not make the current wrapper use it.

- Minime: PID **45642 -> 86528**; readiness verified at 20:04:25Z. No interrupted-job
  recovery was introduced; all 86 startup hashes matched the selected inventory.
- Bridge: PID **46597 -> 87077**, started 20:03:14Z, activated/verified at 20:04:17Z.
  Transaction: `/Users/v/other/astrid/.runtime/bridge-deployment/transactions/8ee1c72f65184962b8838d20dbe66884`.
- Drain checkpoint SHA-256:
  `6ae250b2826199ef2eac6b8cd7671b584f22769fdafaa0dd334cb0f8a4fe8cea`.
  Runtime-schema decoding and self-control lineage passed. Exchange count advanced
  from 207132 to 207133. These receipts do not claim lossless remote delivery.
- Manifest SHA-256:
  `c195860bdf9cabce436099d613661199de83a023ba12aea70526a293820f1805`.
  Bridge SHA-256:
  `8444a4fb4017ea5cec68001c18fdede50c5b899d0240f8f61ed1f60f295e28f4`.
  Shared helper SHA-256:
  `cc31bbd31551b11a3e91c66c9ee4524d62a0ca1ac4dfc0d708113c063856d5be`.
- Minime session 5318 continued, cycle 42776 -> 42777. The pre-stop NEXT hash
  `79bc5dc1b357d0cec9c3ea58612ba277b981ee79b713bc0a5517901a80da4539`
  matched the resumed action's `raw_next` in the 13:06:33 local action manifest.
  Only metadata/hash was inspected, not its authored body.
- At the initial post-start check, Minime had naturally written schema 9, retaining
  50 bookmarks. Astrid retained schema 8 and 78 bookmarks pending ordinary reader
  use; migration was not forced. The old schema-8 helper is not a rollback writer
  for upgraded state. No backup was restored over newer authored records.

`post-live-observation.json` verifies selected helper, source hashes and protected
process identities. Thirty one-second health reads contained 13 distinct snapshots,
maximum age 2.283 seconds, fill 37.159-51.830%, target 68%. A later read showed
65.432%, then 70.282%, with the hard-recovery write block false and no intervention.
Do not call the first sample target-aligned, nor infer that this release caused the
variation. This is operational observation, not experiential improvement.

An old-agent writing job failed at 19:58:41Z, before the 20:02:20Z stop signal.
Its runtime-only private notice identified the existing 48,000-byte full-input
allowance: the draft remained saved and no text was shortened. No authored private
passages were opened. This is the specified honest-overflow behavior, not restart
loss. A voluntary, revision-bound continuation-window design is separate work;
silently summarizing or raising limits was not included in this rollout.

### Separate native kernel and CLI

`kernel-build.log` records a locked release build using a non-live build cache;
qualified outputs were copied into `kernel/`. The installed executable was not a
build target. `qualify-kernel.py` reuses the isolated native harness and additionally
requires typed lifecycle evidence from the release binary.

Fresh-state, actual-old-to-new synthetic-state and freshly stopped-live-state copies
all loaded the seven installed capsules, passed management/rate-response checks,
reported typed advisory readiness, and exited with code 0. Sandbox checks denied
IP networking, live-home reads and out-of-fixture writes. The stopped-state copy
included only var/keys/etc, not journals; no copy was restored into live state.

`kernel-transition.py` binds exact source hashes, binaries, installed capsule assets,
launch configuration and eleven protected process identities. It requires real old
process exit and launchd exit code 0 before job removal or installation. It contains
no force fallback; the earlier consumed September-22 approval was not reused.

Old native daemon **97698** acknowledged shutdown, unloaded all seven capsules,
flushed KV, removed socket/token/readiness files and exited with code **0**. Existing
launchd configuration does not respawn successful exits. Only then was the stopped
job removed, state copied/qualified, and the replacement atomically installed and
started via the existing wrapper. No launchd configuration was changed.

- New daemon **87998**, started **20:05:55Z**, SHA-256
  `b59ce913f978b7e1b579296a9aa6cd4e20468d2fc5355e7bd32669d047f6135b`.
- CLI SHA-256 `7458c70771a888c2cfa6967208e5d2dbeb6e174884914074f7601a9d974a1bb9`.
  The CLI was also updated from the older installed 0.5.1 executable to this
  qualified 0.5.6 build. Against the old daemon it truthfully showed unavailable
  lifecycle observations; against the new one it displayed the actual report.
- Initial discovery: seven loaded/ready, advisory=true, observed at 113 ms uptime;
  zero unacknowledged unloads this boot. This is not continuous readiness or proof
  that every future unload will succeed. Independent daemon exit was verified by
  the operator harness, not inferred from `child_exit_verified` on capsule records.
- `kernel/transition-stop.json`, `kernel/stopped-state-qualification.json`,
  `kernel/transition-activation.json`, and `kernel/cli-activation.json` retain receipts.
  Rollback binaries and stopped-state assets are protected locally; there was no
  automatic rollback. Older writers cannot overwrite newer authored state.

### Git and remaining boundaries

Integration uses separate `codex/lifecycle-release-20260924` branches and exact
owned-path lists: 22 Astrid paths and four Minime paths. The source candidate remains
pinned so future stage verification can reproduce its build identity. Canonical
source is checked byte-for-byte against those inputs before committing. Final merge
identities are recorded in the external `merge-verification.json`; the bridge's
`committed_as.json` binds the dirty-at-build inputs to their containing commit.
No push is included in this requested local merge. Historical foreign work remains
untouched rather than being swept into the release.

Remaining work is the separately reviewed sensory instrumentation and, optionally,
drain-aware deployment scheduling and voluntary long-draft continuation windows.
There is no outstanding release-test failure or restart debt for the activated scope.
Natural use may provide further signal; no confirmation of improvement was requested.
