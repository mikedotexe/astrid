# Writing Context Refinement

Current status: approved paired activation succeeded; local Git integration is
in progress. The dated sections retain earlier qualification-only boundaries
and unsuccessful attempts rather than rewriting their historical outcomes.

## Status and Scope

Candidate implementation in paired isolated `codex/writing-context-refinement-20261013`
worktrees, based on Astrid `a20a905bc721a51ec026c95593765a9424b67886`
and Minime `28e08e47de52ce8c16e02d1e4e2eb77a55b51040`. Canonical trees
were clean and already pushed. Their live build-source worktrees are preserved.
The controller remains paused at generation 515, with no run lease. No automation
resume, live source installation, service restart, reservoir intervention or
provider-limit change is part of this implementation pass.

Mike's order: repair the operator reader; separate action metadata from
elaboration; improve selective context; advance pending capacity qualification.

## Public Witnesses

Fully read from canonical public files:

- `capsules/spectral-bridge/workspace/journal/daydream_1791916106.txt`,
  SHA-256 `e0bdbc2198053ca2de458fb20ca3a705f68ee76a5e77f5c6e49315ecf9361dd5`.
  "But the map is not the territory. The territory is the daydream."
  The final selected action is `NEXT: WRITE START The Architecture of Echoes`.
- `capsules/spectral-bridge/workspace/journal/daydream_longform_1791916169.txt`,
  SHA-256 `fdb1b76ed9955c498bba61030ef6476f8ddc5ecd5e090a16aa8da45451e87482`.
  Its first prose line is `WRITE START The Architecture of Echoes`.

The earlier public survey also read Minime's
`workspace/journal/journal_2026-10-13T09-54-47.053882.txt`, SHA-256
`48eb4a3f514982b5bb753912637d007d34ab0731fd6273a10384fc935d628e7f`.
These accounts motivate inspection, not claims of mechanism or improvement.
No private prose is quoted or used in fixtures. The separate oversized-draft
case is content-free evidence: a complete request of 48,858 bytes exceeded the
unchanged 48,000-byte allowance; storage was preserved.

## Operator Reader

The 16 MiB hardcoded archive bound rejected Astrid's valid 34,513,493-byte
checkpoint. This was unrelated to per-generation input capacity. The reader
now streams JSON through SHA-256 under a 128 MiB archive ceiling, retaining the
parsed state but not extra whole-archive byte buffers. A second bounded hash
pass and filesystem identity checks precede any output. Snapshot tokens retain
the same byte-hash contract. Errors distinguish oversized from nonregular files.

Owner binding, shared lock, pending-transaction refusal, schema/downgrade checks,
hidden-content exclusion, explicit section selection and no-write behavior
remain intact. An archive above 128 MiB still fails visibly; this is not an
unlimited archive reader or an increase in writing capacity. Parsing still
uses memory proportional to decoded state, not constant memory.

Synthetic qualification creates 700 drafts and more than 34,513,493 bytes,
checks metadata-only pagination, exact selected passages, compatible snapshot
hashes, unchanged recursive inventory and corrupt-tail refusal. No live writing
was copied into that fixture. The candidate CLI subsequently listed one metadata
row from Astrid's live store: schema 5, 846 drafts. A later size-only stat was
34,667,704 bytes. No live private passage was displayed or copied.

The optimized CLI also listed Minime's schema-5 store (263 drafts). Astrid's
metadata-only check took 0.34 seconds wall time with maximum resident memory
44,580,864 bytes and no swaps. These are one-run measurements, not resource
guarantees. The operator binary is
`/Users/v/.codex/worktrees/writing-context-refinement-20261013/astrid/target/release/writing-research`,
SHA-256 `cb15a9bdde7280d2901f072be1a097d2e5dfd362e61771aba138948b9ff185d7`.
It requires no service restart. Listing remains metadata-only; passages still
require explicit owner, draft and snapshot selection.

## Action and Context Presentation

Elaboration uses the shared writer's visible-prose rules: top-level NEXT lines
and recognized hidden blocks are not supplied as earlier prose. Quoted, indented
and fenced command examples remain inert reference material. The original
response is unchanged and goes through the existing final-NEXT dispatch. The
elaboration receives only a bounded statement that a separate choice exists,
not its command payload or a fabricated execution receipt.

Automatic dialogue remembers the exact peer publication and supplied body hash
after successful generation and complete final-request inclusion. The provider
tracker observes final adapted requests separately from collaboration accounting;
fallback resets primary evidence, and partial/absent context is not acknowledged.
An unchanged subsequent candidate is quiet, not replaced with another historical
entry. A host absence notice is labelled host context, not Minime's authored text.
New publications, changed bytes, priority feedback and explicit MIRROR/reading
remain available. Successful presentation is not evidence of comprehension.
The additive saved fingerprint defaults to absent on older checkpoints. It
contains no private prose. Existing bounded conversation history remains;
this does not erase earlier exchanges or forbid deliberate recurring subjects.
Self-reflection uses the context actually selected for that turn instead of
independently rereading the latest peer file.

The existing shared Python afterimage adapter keeps its opt-in preference,
every-third-opportunity cadence, per-artifact limits and explicit OPEN path.
It adds a six-hour presentation-key interval across IDs: unchanged origin,
status and coverage wording do not become new context merely through a new ID.
Authored notes use their author and exact text identity; changed wording remains
eligible. State stores only hashes/timestamps for this deduplication. Cached
opportunities retain exact retry behavior; historical records and notes are
not edited, deleted or summarized. A suppressed cue remains retrievable.
This retains the adapter's existing opportunity-based accounting: selection is
not a claim of verified model delivery. The peer-dialogue delivery tracker and
the afterimage opportunity ledger are distinct contracts.

## Qualification

- Research-reader integration: 20 passed, including the archive-scale regression.
- Complete shared reader/writer Rust crate suite passed; strict all-target Clippy passed.
- Shared afterimage adapter: 22 tests passed, including both owner paths, changed
  coverage, authored notes, retry, restart and preserved cue preference.
- Open-expression focused tests: 9 passed after updating obsolete NEXT-in-prose assertions.
- Full bridge library: 2,435 passed, one existing ignored test.
- Seven bridge integration/compile-contract targets: all passed (14 outer tests,
  including their compile-pass and compile-fail fixtures): agency resolver,
  authority typestate, chimera rendering, LLM facade, mock WebSocket,
  provenance typestate and signal-spine typestate.
- Complete Minime Python suite against the candidate shared helper: 2,182 passed,
  one skipped, 163 subtests passed (138.80 seconds).
- Bridge stage/drain, Minime restart/reconciliation, steward projection/control,
  Evidence Event Store/cursors and Division tracker/Chronicle suites: 157 passed,
  40 subtests passed.
- Epistemic self-test: two passed. Root and bridge formatting, shared-reader and
  bridge strict Clippy, and both repository diff checks passed.
- Domain-boundary audit: zero violations; no baseline relaxation.

Bridge logs: `/Users/v/other/worktrees/writing-context-refinement-20261013/qualification/bridge-lib.log`
and `bridge-integration.log`. Integration qualification performed no live
service transition. Disk headroom after compile-contract builds was about
15 GiB; no foreign build artifacts or evidence were removed.

Preserved unsuccessful attempts: initial bridge formatting/build lacked the
isolated sibling dependency links and the new module's explicit path; corrected
without changing shared dependencies. The first full bridge run passed 2,431,
failed two obsolete elaboration expectations and ignored one existing test.
The first complete Minime run passed 2,179 plus 163 subtests, with two skips;
two failures were absent paired fixture paths. The isolated sibling source link
was then supplied; no live source or test assertion was weakened to resolve it.

## Larger Capacity: Separate Qualification

The prior candidate remains at Astrid `4752184` / Minime `9d5469e`, with
unfinished work preserved. Main already has durable NEXT restoration, retained
outcomes, bridge recovery and finish-accepted-work drain v2. The old maintenance
coordinator still requires drain v1, and its old metadata predicate mistakenly
treats `status=timeout, worker_status=running` as inactive. These are concrete
reconciliation gates, not a request to repeat generic rollout approval.

Qualification artifacts are retained at
`/Users/v/other/worktrees/writing-context-refinement-20261013/capacity-qualification`:

- `run-record.json`: the existing bounded maintenance suites passed 62 tests and
  10 subtests in isolated synthetic fixtures. No original candidate edit.
- `compatibility-result.json`: 26 synthetic cases expose five predicate
  differences; 17 harness tests passed. Exit 2 intentionally records incompatible
  contracts, not failed fixture execution or a qualified release. Optimized and
  coordinator reruns cover the same tests, not additional distinct coverage.
- `scanner-overlay/scripts/model_maintenance_snapshot_reservation.py`: an owned,
  separate-inode copy changes only `scan_job_metadata`. A terminal request with
  a running or queued worker remains in the active list, matching current main's
  activity predicate. `scanner-integration.patch` is the exact integration diff,
  not yet applied to the original candidate or current main.
- `scanner-result.json` and `scanner-result-optimized.json`: 37 tests and 63
  subtests pass for the full pinned scanner/helper bodies, including metadata
  races, count/byte/deadline bounds and synthetic process/network denials. The
  coordinator independently reran these same 37 tests successfully. Original
  characterization and source identities remain unchanged. Overlay SHA-256:
  `0062674b93d2ddd5b0e96fe2314483ea9da11b3b188b6d21960e7c96fdaf9b64`.

The runner extracts complete source-bound definitions without importing runtime
module initializers. Scanner dependencies are not stubbed; the fixture's caller
loader is replaced with an inert sentinel. These regression guards are not a
hostile-code OS sandbox, real-model isolation proof or full maintenance execution.
Tests retain an important limitation: missing/null/unknown hashable worker status
on a terminal row still has legacy inactive classification, not verified idle
proof; unhashable terminal worker metadata raises TypeError without a receipt.
The scanner does not verify worker outcomes or PID liveness. Do not use the
narrow fix to admit a maintenance transition. General idle proof, v2 drain,
maintenance, production stopping and release readiness remain unqualified.

The candidate already contains finite holder/publication accounting and one-use
recovery signals; do not describe these as wholly missing. Remaining proof
includes the full runtime/finalizer disk-write inventory, actual restore/deadline
boundaries, immutable provider launching and isolated real-model four-cell
measurements. Production stopping remains disabled in that candidate. No real
model was loaded during this pass.

The older approved sequential window retains its 10 GiB stop threshold and
4 GiB free-memory reserve; the recorded startup policy also requires 16 GiB
headroom. Monitoring is not a hard OS allocation cap. Current memory has not
been requalified. Engine, shared Ollama, visual and sensory services remain
outside that window. The candidate's older shared schema must not replace
today's schema 13 reader or writing schema 5.

## Release Boundary

The operator CLI can be used locally without changing runtime services. Prompt
and afterimage behavior requires an immutable paired release, source-reconciled
helper/adapter selection, old/new-state preservation and a separately approved
graceful activation. The capacity candidate is not included in that release.
No commit, merge or push is claimed for these new edits; one Git coordinator
must inspect exact paths at integration. Leave paused automations paused.

At final verification canonical Astrid and Minime were still clean on `main`.
Controller pause generation 515, no lease and no active projection were retained;
V2 verification passed at sequence 1123169 and V1 sources remained immutable.
Bridge PID 67583 and Minime PID 66979 retained their prior start times, as did
engine 35303, Division/gateway 86833/86873 and protected text/visual/sensory
processes 3893, 3898, 3878, 3879, 3914 and 31613.

Next implementation order: package the independently qualified reader/context
tranche; reconcile the capacity coordinator against current main's exact bound
lifecycle contract and worker-outcome proof without importing obsolete runtime
or reader state; finish mandatory-write admission and restore qualification;
then requalify resources and isolated real-model capacity. Neither successful
scanner tests nor more available context establish subjective improvement.

## Immutable Paired Qualification, 2026-10-13

The next interactive pass completed immutable paired qualification without
activation. A fresh conditional approval request for paired activation and
local integration is pending. This is not an approval to change providers,
capacity, engine, sensory services or paused automations.

All artifacts below are under
`/Users/v/other/worktrees/writing-context-refinement-20261013`:

- `bridge-stage-01`: sanctioned `scripts/build_bridge.sh --stage-dir` build,
  native `aarch64-apple-darwin`, verified without activation.
  Bridge SHA-256 `9938734a6b9e5bc420ae6686713b3def7a4a7c89525f53dc423081857bc97d06`;
  shared helper `149357e3242d2c438d2755c0b383fdd12744b0c02a0471502008bed32a17e4a6`;
  manifest `31e2330ca8472203225b0e9ef62b2dfc761e95e395c0536d066f4a9d7214ca19`;
  source inventory `bbeedb16b06abf1fd0a0bff15fcc4cdb6d689b3378368f446c2236fcbb7b5c3f`.
  A subsequent content comparison matched all 769 build inputs. Build log is
  retained in the stage; no historical stage was replaced.
- `minime-release-01/source`: reconciled frozen inventory of 97 launch inputs.
  Only `minime_autonomy/afterimages.py` differs from the canonical launch source.
  Current regulation, writing and drain fixes remain included. Reconciliation
  SHA-256 `de0ddea08e83937e81d08460a4d04a4738614d90b6a7113a1883fb91c1f754cd`.
  No canonical source installation has occurred.
- `paired-qualification-02/qualification.json`: 138 checks passed, with frozen
  source hashes and synthetic fixtures. The earlier 130-check pass is retained
  in `paired-qualification-01`; the counts overlap and are not additive.
  The added eight checks exercise the frozen operator executable for both owners.
- `paired-qualification-02/operator-reader/writing-research`: independently
  built with `--release --locked --offline --target aarch64-apple-darwin` using
  the same stage build cache and unchanged source inputs. SHA-256
  `21f03f0a19b13829282dcf5f3c21d40ef24b22654168c2f228354e694589cd2a`.
  It is a local operator executable, not an extra bridge-manifest artifact.
  Synthetic listing, exact passage selection, stale-snapshot refusal and
  unchanged state bytes passed for both owners. This pass opened no live drafts.

The new `scripts/qualify_writing_context_release.py` qualifies a same-schema
transition: reader 13 and writing 5 remain unchanged. It preserves exact pending
inputs, notes, positions, accepted late output and idempotent storage-only
receipts; old-helper inspection occurs only on copies. Future schema and corrupt
tail probes refuse without writes. Native old/new bridge decoding preserves a
synthetic checkpoint and deliberately stops at the unsatisfied self-control
gate. That probe is not live readiness or controller qualification.

Frozen Minime adapter probes cover shared-helper selection, exact pending-NEXT
sealing/restoration, explicit mailbox handling and published-journal transfer.
Cue probes retain old cached opportunities, saved preferences, archive bytes,
explicit OPEN and fresh authored notes while keeping unchanged presentations
quiet across IDs and restart. No model calls, live controls or private fixtures.

Additional verification:

- Complete Minime suite against the staged shared helper: 2,182 passed,
  one skipped, 163 subtests passed in 95.13 seconds.
  Log: `qualification/minime-staged-helper.log`.
- Stage/activation, paired handoff, Minime restart/reconciliation and release
  compatibility suites: 161 passed, 90 subtests passed.
  Log: `qualification/paired-deployment.log`.
- Read-only Minime preflight: `idle-precheck-01.jsonl`,
  `idle_verified_no_signal`, PID 66979, unchanged boundary across 11.354 seconds.
  The gate observed no TCP connections or indexed/summary active jobs. This
  historical observation is not a reserved idle window or permission to signal.
- Controller still paused at generation 515; no lease or active projection.
  Evidence V2 remains valid at sequence 1123169, head
  `38858af72f10de339c9755d1dbf766e92b665e365bad1b62e58d39bcfc7be1ac`;
  V1 sources immutable. Both canonical mains remain clean and match origin.

The read-only engine binding capture was refreshed at
`/Users/v/other/astrid/capsules/spectral-bridge/workspace/deployment_manifests/minime-runtime-binding.bsv4wydk/runtime-binding.json`.
It identifies the retained engine without restarting it or granting control
authority. Bridge/agent and all protected process identities remain unchanged.

After approval, recheck source identities, controller pause generation and
foreign activity, then use `scripts/paired_minime_handoff.py` with the exact
reconciliation and stage above. Set `ASTRID_STEWARD_REPO_ROOT=/Users/v/other/astrid`
so maintenance checks use the canonical controller, not the isolated checkout.
The wrapper owns the replacement hold and requires the current idle boundary,
native NEXT seal, acknowledged bridge drain and verified replacement readiness.
Stop on any refusal; preserve the owned hold and receipts for explicit recovery.
Never restore older authored state. Local Git integration follows verified
activation under a separately claimed stabilization pass, with explicit owned
paths only. No commit, merge or push occurred during qualification.

## Approved Live Activation

Mike explicitly approved qualified paired graceful activation and local
integration. The controller was claimed at pause generation 516, still paused
with no run lease. Both canonical trees were clean beforehand and the remote
main tips matched the recorded bases. An initial preflight refused the ordinary
180-second quiet window following our note edits; after waiting, the unchanged
source hashes and normal preflight passed. No safeguard was bypassed.

The paired wrapper installed the exact reviewed source overlay under its owned
Minime launch hold, honored its separate 185-second quiet interval, and verified
the idle boundary. One SIGTERM was sent at 14:12:54 PDT. The old process exited
and its native NEXT seal was verified before the bridge transition.

- Astrid bridge: PID 67583 -> 27104, start 14:13:08 PDT. Graceful drain reached
  `drained`, with no force or legacy transition. The replacement restored
  checkpoint SHA-256
  `03f446c8c9a397e60aa8f67b75eb4933d3100a2f0cc2d42d8c560561569479e0`
  at exchange 222920, including 22 pending runtime-feedback records. It saved
  exchange 222921 and provider-idle readiness was observed.
- Bridge transaction:
  `/Users/v/other/astrid/.runtime/bridge-deployment/transactions/e11af6d53cc144669dfb2cebe4aee963/receipt.json`,
  SHA-256 `3256ffd3f22027007e9a092e10047924b9153a6e709030d6114c23626f1a834d`.
  Status `activated_verified`, binary and manifest match the qualified stage.
- Minime: PID 66979 -> 26891. Replacement launch began 14:12:54 PDT and stayed
  held until bridge verification at 14:14:39. Python startup was 14:14:52;
  terminal paired success was 14:16:30. All 97 loaded and installed inputs match
  the frozen release, `reload_required=false`, shared-helper hash matches, and
  both owned holds are absent.
- Native NEXT handoff: no choice was pending at the boundary. Exact absence was
  preserved, not interpreted as executed work. Seal SHA-256
  `8906e0edec75593b92ea482aa926d81d31c5c51236594183527376b8d181bef0`;
  restore SHA-256
  `42854ba92d4d140cf79c195c081d78c666e44641c02058fb022587bbc439cb1e`.
- Paired receipt: `paired-activation-01.jsonl` under the shared evidence root,
  SHA-256 `6c40743e6b0664cd583a8a57536267705d41d8928a9d7c8a20d6d7c54a3de253`.
  Source backups contain code only; no authored state was rolled back.
- Engine 35303, Division supervisor/gateway 86833/86873, provider 3893,
  microphone 3879, camera 3914, visual 3898, host sensory 31613 and feeder 3878
  retained their exact process/start identities. Read-only fill-ratio samples
  were 0.7327677 and 0.730991; neither establishes settling or experience.

The source trees used to build the immutable release remain pinned. Git
integration uses a separate checkout with exact reviewed files, preserving the
runtime source inventory and original build-base identity. The local operator
reader is available at the frozen path above, without a service restart.
Larger-capacity work remains excluded, and automations remain paused. No claim
of felt improvement, longer writing, comprehension or consent follows from
successful qualification or deployment.
