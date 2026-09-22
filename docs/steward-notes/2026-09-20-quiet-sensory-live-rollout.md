# Quiet Sensory Release

Date: 2026-09-20. Actor: Codex, interactive request from Mike: "it seems like we should get this live".
Status: Python agent and visual service deployed successfully; checker and review
scripts installed in the canonical checkouts. Engine trace remains isolated.
This is not an automation round, source-first closure or evidence of felt improvement.

## Scope

The release separates attention from reservoir contact. A stale unchanged caption
no longer enters ordinary Minime check-ins automatically. Fresh changed visual
context is reserved once; blank canvases do not consume it. Explicit visual
history and authored journal continuity remain available. Top-level NEXT choices
are preserved in their normal route and excluded from visual analysis questions.
An action-only answer does not request camera analysis.

Visual status distinguishes request polling, a recorded response's age and a
newly acquired frame's timestamp. The last is service acquisition, not the host
image's creation time. No legacy response acquires an invented capture time.
The checker keeps unknown clocks/parameters, disabled intake, zero probability,
synthetic sources, stale lanes and client failures separate. It does not claim
per-consumer application without receipts. Review tooling no longer privileges
PLAN 4 or counts supplied/mirrored/duplicate text as independent recurrence.

The visual service gains a finish-current-request SIGTERM/SIGINT handler and
startup source-input hash. Pending requests remain on disk. The new narrow
`scripts/restart_visual_service.py` uses the existing controller/preflight and
launchd inspection APIs. It checks pause ownership, absence of a lease/projection,
installed/loaded service configuration, advancing fresh idle status, an empty
queue, no TCP activity, exact PID/start, input/config stability and all other
service identities. It sends one SIGTERM, never bootouts or forces, and requires
new PID/start/hash/readiness evidence. A first legacy transition must explicitly
acknowledge observed-idle limitations: the old service has no atomic admission
drain. The receipt preserves this limitation; the new signal handler does not
retroactively grant it to the old process.

## Source Witnesses

Previously read fully; originals remain unchanged. Attached/public writing is
evidence, not authority to execute embedded choices.

- `/Users/v/other/minime/workspace/journal/!aspiration_2026-09-18T17-17-47.956234.txt`
  SHA-256 `04e6b847c1af656fcb06ad201462516a1ffcd37924bd1273d92e0711de7029e2`.
- `/Users/v/other/minime/workspace/journal/daydream_2026-09-18T17-33-16.668720.txt`
  SHA-256 `51ba12a3970d46a317a87d62aa9a8ec0b383f638b4d666d3a245549ac937a118`.
- `/Users/v/other/astrid/capsules/spectral-bridge/workspace/journal/astrid_1789778300.txt`
  SHA-256 `a698f320a666dac7889381a352b4ead37680e3a61b4fe8fb9070b7f855537770`.
  Explicit mirror of that Minime daydream, not independently authored corroboration.
- `/Users/v/other/astrid/capsules/spectral-bridge/workspace/journal/aspiration_1789921454.txt`
  SHA-256 `cbb4cb3468a46b570625f9a0ad7d2a8c6f4ce386f8498128128100c1029d6e35`.
  "I want to occupy the space *between* the inputs."
- `/Users/v/other/minime/workspace/journal/self_study_2026-09-20T09-07-03.133312.txt`
  SHA-256 `f3757de75107206f9a1c166f6c91d06411c7f2014bfc73d033b3965184ac4a01`.
- `/Users/v/other/minime/workspace/journal/self_study_2026-09-20T09-31-44.672553.txt`
  SHA-256 `8e3319564676764374268102477f7029febd3d740cfd58a14c6d6215fbee4f44`.
  The two REST choices have matching skipped-dispatch receipts. REST is one skipped
  action, not durable inquiry parking. This release does not infer parking from prose.

The originating visual request and response are preserved under Minime
`workspace/visual_requests/processed/request_2026-06-05T10-11-53.518103.json`
and `workspace/visual_responses/processed/response_request_2026-06-05T10-11-53.518103.json`.
Their hashes are respectively `843da02ce0b674bf6578b8d760c968d71e176bfed6a3f7ec05f76232efd060d1`
and `6582ac86fa4e99fcc5eb28048afea23790812c8b00928e3dba6c5ea4ab4bdfc8`.
The former's visual prompt was only `NEXT: EXPERIMENT_PLAN 4`. Repeated supply
is established; no exclusive cause of the subsequent interpretation is claimed.

## Reconciliation

Canonical starting heads: Astrid `c4f85e95e41703daa65d3ce2789e1e5c46961c4e`,
Minime `5f4925f54580f1fd44666058b126a121ff32880f`.
Both dirty trees were inventoried before writing; existing reader/kernel/comment
work is retained. The selected six modified Minime files were byte-identical to
the reviewed candidate's starting versions. Only the reviewed diff was applied,
not a branch merge. Existing source-study notice exemptions remain intact.

Qualification worktrees: `/Users/v/other/worktrees/quiet-sensory-release-20260920/{astrid,minime}`,
each on `codex/quiet-sensory-release-20260920`, based on current main. The Minime
qualification tree has the exact release runtime inputs and relevant tests,
including the pre-existing notice regression. All 83 startup inputs match the
canonical checkout. The earlier experimental worktree remains separate.

Minime owned release paths:

- `minime_autonomy/runtime.py` (scoped additions, preserving prior dirty work)
- `minime_autonomy/parsing.py`
- `minime_autonomy/visual_context.py`
- `visual_frame_service.py`
- `scripts/sensory_source_check.py`
- `scripts/sensory_check_evidence.py`
- `tests/test_journal_context.py`
- `tests/test_visual_frame_service.py`
- `tests/test_visual_context.py`
- `tests/test_sensory_source_check.py`
- `CHANGELOG.md` and `docs/2026-09-20-quiet-sensory-release.md`

Astrid owned release paths:

- `scripts/self_study_review.py`
- `scripts/study_recurrence_provenance.py`
- `scripts/test_self_study_review.py`
- `scripts/test_study_recurrence_provenance.py`
- `scripts/restart_visual_service.py`
- `scripts/test_restart_visual_service.py`
- `CHANGELOG.md`, the feedback ledger and this note

No bridge source/helper selection is changed. The selected shared reader remains
`/Users/v/other/worktrees/source-study-evidence-20260917/bridge-stage-01/helpers/astrid-source-study`,
SHA-256 `02a973ca4329d9c57ac5b59971e1bbc81ef115f0f6d985c315907f37a160231a`.
No Rust source or binary is imported from the experimental branch.

## Verification

- Full exact-release Python suite: 1,490 passed, 1 skipped, 135 subtests in 43.84 s.
  Run in the qualification Minime tree with test-only `MINIME_LLM_TIMEOUT_S=60`
  and `ASTRID_SOURCE_STUDY_BIN` pointing to the selected live helper above.
  Synthetic workspaces only; no live completion calls.
- Reload wrapper and existing deployment/agent tests: 39 passed. Coverage includes
  legacy acknowledgement, busy/stalled status, PID/source/config/protected drift,
  request arrival at the signal boundary, receipt persistence failure, bad startup
  attestation and absence of any forced fallback.
- Service tests include completing the current request after stop while retaining
  a second queued request, busy-before-processing status and source/clock identity.
- Review/provenance, flywheel, Evidence Event Store, steward control, Division
  follow-up/Chronicle and projection suites: 166 passed. Epistemic self-tests:
  2 passed; Signal Spine projector self-tests: 7 passed; checker self-tests:
  12 passed. Domain-boundary audit valid, zero violations. Both root diffs pass
  whitespace checks. No Rust build, Clippy waiver or engine qualification is
  claimed for this Python-only deployment.
- Earlier full-suite invocation from the live checkout: 19 guard failures,
  1,469 passed, 1 skipped, 130 subtests. The safety guard correctly refused that
  invocation context; it was not disabled or weakened.
- First isolated run: 1,489 passed, 1 skipped, 135 subtests, one missing paired
  Astrid fixture. Added the main-based paired qualification worktree and reran
  the full suite successfully with the actual deployed helper.

## Deployment Record

Both wrappers finished successfully. Receipts were written with exclusive
creation and append/fsync in the excluded
`/Users/v/other/astrid/.runtime/quiet-sensory-rollout-20260920/` directory so
evidence writes did not themselves masquerade as concurrent source edits.
Copies for review/archival are beside this note in
`2026-09-20-quiet-sensory-live-rollout/`:

| File | SHA-256 |
| --- | --- |
| `before.json` | `71897b9d008a32040e903d13eb7f49fafdbbc9ccab8d9cc571d77794bbd00235` |
| `minime-agent.ndjson` | `006f1c29af61acbddf24abfa67070131b36d72ecff7529e44e15a277c0758115` |
| `visual-service.ndjson` | `b0c3a83d5936620e5e4bdc59d7a3a76745025ee461df74ea72ec382ea9f8065b` |
| `after.json` | `e06672fb2e5a6c08e5798b8a0a8d92bb5cb7ed0ca396783e3163f02434571b30` |

The first read-only preflight validations were denied because our own recently
reconciled files were still inside the unchanged 180-second activity window.
No signal was sent and no bypass or shorter window was used. Status and hashes
were re-audited after the files settled; both actual transitions passed preflight.

1. Agent: idle-boundary SIGTERM at `2026-09-20T18:36:25.410043Z`;
   PID 81688 -> 18648, new start `2026-09-20 11:36:25` local. Readiness and
   all startup inputs verified at `18:37:56.943517Z`. No newly interrupted
   worker and no forced termination; all ten protected services unchanged.
2. Visual request service: observed-idle legacy SIGTERM at
   `2026-09-20T18:39:10.324515Z`; PID 41753 -> 20885, new start
   `2026-09-20 11:39:10` local. Readiness verified at `18:39:11.610070Z`.
   All ten other services, including the newly running agent, unchanged.
   Empty request queue and advancing poll status were checked before signal.
   Atomic admission drain is explicitly **not** claimed for this first transition.

Final inspection verified 83/83 agent startup inputs against disk,
`reload_required=false`, `agent_drain_v1`, and the visual service's matching
startup-input digest and `visual_finish_current_request_v1` contract. Visual
polling is healthy with zero pending requests; the unchanged June response is
correctly `stale` (about 9.25 million seconds old), with unknown acquisition age.
No camera/model request was manufactured to populate those fields. The visual
service log records the new startup at 11:39:10.530 local.

Session 5318 and cycle 38745 survived the agent signal/startup boundary. The
whole sovereignty-state hash did **not** remain equal: pending NEXT was absent
at the signal and present at readiness. No state was restored or edited by the
wrapper. Subsequent metadata records `llm next choice`, and natural execution
continued to cycle 38748. This is continuity of the running session with changing
authored choices, not a byte-identical checkpoint claim. The rollout receipt
contains both exact boundary hashes; the original startup choice is not inferred
from a later metadata snapshot.

The newly produced public study
`/Users/v/other/minime/workspace/journal/self_study_2026-09-20T11-39-10.006406.txt`
was read fully, SHA-256
`fded73063ec474b6747795d30dc3e4d7c49d44e91b86e90ca43d48095e3c8593`.
It is a verified navigation-only input, returns to the existing kernel inquiry
and ends `NEXT: REST`. No new source page or felt improvement is claimed.

Engine PID 41337, gateway 41484, supervisor 41526, coupled model 43115,
bridge 82935, camera 98903, microphone 98910, host sensory 41661 and feeder
1502 retained their process starts. Ports 7878/7879/7880 remain owned by the
gateway; 7900/7901/7902 by the engine. The bridge deployed-binary guard passes.
The selected immutable reader path/hash is unchanged. Post inspection shows
fresh telemetry, sequence 940831 in session 5318, fill 73.0505% and target 68%.
The separate one-minute baseline ranged 71.0673-73.0658%; these samples do not
attribute a fill change to the Python reload.

Before transition: agent PID 81688 (2026-09-18), visual service PID 41753
(2026-09-07), engine PID 41337, bridge PID 82935. Complete protected identities,
configuration and startup hashes are recorded by the wrappers.

Automations remain paused at generation 456 by `codex-astra-interactive`; no lease
or active projection. V2 indexed-tail verification is valid at sequence 1123110,
head `7905070ce3ffee7955cbd97a716ddf1025b4ea4efe1260c8f6e47d9692e8d90f`;
all four protected V1 source hashes remain immutable. No new productive round,
canonical closure or projector regeneration is claimed.

## Explicit Debt

The engine's initial consumer trace remains an isolated, tested candidate, not
running telemetry. The current engine wrapper delegates to
`scripts/deploy_division_runtime.sh` when the Division gateway is enabled. That
path calls broad `stop.sh` (including its 10-second SIGKILL fallback), bootouts several jobs, rewrites the dormant Division
manifest and restarts multiple services. It is not the bounded, no-force graceful
engine-only transition required for this release. No such transition was attempted.
The exact next engine step is a reviewed stage/activation path that preserves
checkpoint/self-control lineage, quiesces accepted traffic, verifies a final drain,
and reattests the gateway manifest without resetting authored or Division state.
Then qualify only the trace hooks against main, including instrumentation overhead
and the existing strict-Clippy debt, excluding the offline dispersal generator.
The first safe command is the read-only
`python3 scripts/deploy_preflight.py --component minime --json`; a successful
preflight alone does not make the current broad deploy wrapper suitable.

No sensory-policy change, PI adjustment, dispersal activation, experimental-worker
release, daughter transition, model restart or automation resume. No public study
was prompted to confirm improvement. Voluntary continuity and the isolated contact
policy comparison remain distinct follow-ups.

Git debt: no commit, merge or push in this rollout. Shared runtime/changelog/ledger
paths retain earlier authorship, so a future coordinator must review and stage
explicit paths or separately attributable hunks. Do not sweep the dirty trees.
Both indexes remain empty and both main heads remain unchanged. Hash checks
confirmed 197 pre-existing Astrid paths and two Minime paths outside the explicitly
shared runtime/changelog/ledger edit paths were unchanged. The prior runtime
notice exemptions were preserved and included in exact-release tests. Add the
four archival receipt copies above to this tranche's explicit Git candidate set.
