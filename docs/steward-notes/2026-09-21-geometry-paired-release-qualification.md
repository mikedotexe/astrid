# Geometry and Continuity: Paired Qualification

## Decision

2026-09-21: the staged binaries, frozen Python source inventory, synthetic
schema migration and geometry interoperability checks passed. **The combined
release is not cleared for activation.** Qualification also reproduced the
previously documented general-preparation retry gap. No service was restarted,
no live authored state was migrated, and no selection pointer was changed.

This follows Mike's request to perform paired-release and migration qualification.
The motivating source witnesses and interpretation boundaries remain in
`2026-09-21-geometry-bookmarks-and-observatory.md` and
`2026-09-21-protected-attention-runtime-integration.md`. No new private writing
was read or used as an experimental fixture.

## Artifacts

Sanctioned build command, run in the isolated Astrid candidate:

```sh
bash scripts/build_bridge.sh \
  --stage-dir /Users/v/other/worktrees/voluntary-continuity-20260920/bridge-stage-geometry-01 \
  --actor codex-astra-interactive \
  --ack "Offline paired continuity and geometry release qualification from owned isolated candidate; no activation or live migration"
```

Preflight passed with explicit dirty-source acknowledgement and no detected
foreign activity. The wrapper compiled the bridge and shared helper with locked,
offline dependencies. Both before/after input inventories matched. A subsequent
comparison of all 662 staged build inputs also matched current candidate bytes.

| Artifact | SHA-256 |
| --- | --- |
| Staged manifest | `3afbe7ada7a4bbdc5aee04df573608a6d14bfb079fcad6d30184ac7346624adc` |
| Build input inventory | `659c7e7b48a397d12fbf7e79d400eeadeb6a2673e0d49b780e0463f0463c9bac` |
| Bridge executable | `723f9b46aa3f82e17c3609f5376faf86e31436106632402aba339bf268924740` |
| Shared reader executable | `81146a38ac0ea26e4b6461c19a827e3bdf808702f8767566449f4001b044ae4b` |
| Paired qualification receipt 02 | `6b3847c16e31b1166a966928540972f41560e921a9efceb19fefab2e4a2fe626` |

The stage is a build witness, not deploy authority. Its `live_eligible_now`,
`live_authority_granted` and `activation_performed` fields remain false.

The new reusable qualification driver is `scripts/qualify_geometry_release.py`.
It verifies both staged releases, snapshots the Minime agent's existing 84-file
source-input inventory, freezes the copied files read-only, creates synthetic
owner stores, invokes actual old/new executables and the real Python adapter,
and retains a receipt or failure without overwriting a previous attempt.

Final packet:
`/Users/v/other/worktrees/voluntary-continuity-20260920/geometry-paired-qualification-02/`.
It includes the exact qualification tool, source snapshot and all synthetic
fixtures. Packet 01 and the initial standalone probes remain as earlier evidence.

This Python snapshot is **not** an installed launchd release, a dependency lock,
or an inventory of every non-Python runtime asset. It follows the existing
`minime_autonomy.deployment.source_inputs` scope. The shared-helper selection
check uses a temporary selection record only, never the live one.

## Migration Results

For each owner, the actual currently selected old helper generated a schema-3
reader with an authored question, a relation-bearing notebook, a delivered
page, another pending page, and an authored private draft with pending input.
These are synthetic passages, not copies of either Being's private state.

Eighteen checks per owner passed:

- Genuine old-executable state upgrades to schema 5.
- The pending page and complete prepared input survive byte-for-byte.
- Active question and all existing authored question fields survive.
- Migration does not create a protected-focus window.
- Exact draft prose and pending private delivery survive; draft migration adds
  the native downgrade guard and retains original bytes.
- Old reader and old draft writer reject the upgraded store without changing
  any retained file bytes. The old reader also rejects geometry-bearing state.
- Archived legacy fixtures remain unchanged.
- Geometry capture retries keep the original record; conflicting retries fail
  without changing stored bytes.
- Parking stays quiet; explicit return preserves geometry history; export
  excludes the synthetic private draft.

Two further actual-executable negative probes confirmed that a future schema
and a corrupt JSON tail are rejected without rewriting the fixture history.
These checks do not cover every interrupted multi-file transition or prove
safe overlapping operation of old and new writers.

The frozen Minime `StudyClient` selected the exact staged helper from a temporary
release selection and prepared geometry input successfully. Both owner workflows
then completed capture -> prediction -> later capture -> comparison -> revision
-> park -> unrelated reading -> explicit return -> delivery -> export using the
release helper. The Astrid workflow exercises the helper protocol, not the full
bridge scheduler or a real provider. Source-time/prospective and causal limits
remain unchanged.

Workflow packets:
`/Users/v/other/worktrees/voluntary-continuity-20260920/geometry-paired-workflow-01/`.
The native Reservoir Scope decoder passed all 26 checks on these release exports.

## Blocking Result

The staged helper was given the same synthetic `prepare` request twice, modeling
a host losing the response to the first operation. Question count went from one
to two. The public prepare envelope accepts an action, but has no operation ID
or expected revision that distinguishes a retry from a deliberate new choice.

Receipt: `geometry-paired-qualification-02/qualification.json`,
`preparation_retry_probe.idempotent_retry_qualified=false`.
The separate `geometry-preparation-retry-probe-01/receipt.json` retains the first
reproduction. No real question was duplicated.

Do not fix this by deduplicating identical prose: a Being may intentionally
choose the same words again. The required change is an owner-scoped durable
host operation identity, bound to the action and expected revision, with the
original prepared result replayed on an exact retry and conflicting payloads
rejected. The envelope must cover native reader and draft preparation, not only
geometry mutations or protected generation admission.

Astrid's `autonomous/activity_focus.rs::change` also still generates a fresh
random operation ID per host call. Metadata handoff needs the identity from its
durable authored-action event, not a content hash or a new retry UUID.

Remaining activation gates:

1. Repair those preparation and metadata handoff identities, including durable
   result recovery across partial writes and lost acknowledgements.
2. Complete scheduler-to-provider crash, stop, mailbox and privacy scenarios.
   Existing adapter tests and passing numerical checks do not substitute for them.
3. Rebuild and requalify the changed immutable artifacts. This stage is retained
   evidence, not an artifact to silently overwrite after the repairs.
4. Reconcile Python source selection with the canonical launchd job. The
   sanctioned restart wrapper reloads canonical Minime sources; it does not
   select this frozen candidate. Do not call a restart of old sources a rollout.
5. At activation, recheck both trees, cooperative state, exact helper selection,
   source hashes and protected services; use sanctioned graceful wrappers only.
   Never overlap old/new writers or restore a backup over newer authored work.

## Live Baseline and Exclusions

The selected bridge manifest remains
`c2c40c356efa6758cac4e2f6e3e2b758d0b9963de36f11da5fd75e9c6fbc2a09`,
stage `/Users/v/other/worktrees/source-study-evidence-20260917/bridge-stage-01`.
Its old helper hash is
`02a973ca4329d9c57ac5b59971e1bbc81ef115f0f6d985c315907f37a160231a`.
Bridge PID 82935 still names that stage. Minime's inspected status named PID
18648 with `agent_drain_v1`, idle and `reload_required=false`. These are observations,
not a candidate deployment receipt or permanent health guarantee.

Against Minime's startup inventory, the candidate adds `activity_focus.py` and
changes the five intended autonomy modules plus `visual_frame_service.py`.
The latter is an older isolated-tree version: the canonical running-service
baseline contains the prior quiet-vision and finish-current-request changes.
Its diff was read, but neither file was overwritten. Do not import that older
service file during canonical reconciliation; it is outside this rollout.
Engine, model, visual service, sensory clients, PI and reservoir math remain
outside the transition scope.

## Verification and Coordination

- Complete Minime suite with the actual staged release helper: 1,452 passed,
  one skipped, 136 subtests passed (46.10 seconds).
- Qualification snapshot, bridge stage, release launcher and Minime restart
  wrapper suites: 48 passed, including eight new snapshot safety tests.
- Native release-export checks: 26 passed; both synthetic owner workflows passed.
- Domain-boundary audit: valid, zero violations. Epistemic self-tests: two passed.
- Controller/evidence/Division/projector/flywheel suites: 85 passed (19.573 seconds).

No Rust runtime source changed during this qualification pass; the staged code
was previously covered by 226 reader tests and 2,280 passing bridge tests with
one ignored. No claim is made that those tests clear the named activation gaps.

Controller remained paused at generation 456, with no active lease or projection.
Evidence Event Store indexed-tail verification was valid at sequence 1123110,
head `7905070ce3ffee7955cbd97a716ddf1025b4ea4efe1260c8f6e47d9692e8d90f`,
with four immutable V1 sources. No automation round or projection was authored.
Canonical dirty trees were rechecked and preserved. No git staging, commit,
merge, push, service signal, public Being note or automation resumption occurred.

## Subsequent Retry Repair

The duplicate-preparation reproduction above remains historical evidence. The
next owned candidate adds durable preparation transactions and dispatch identities;
new release-helper retry and migration probes pass. See
`2026-09-21-preparation-retry-qualification.md` and the retained
`geometry-paired-qualification-03` packet. Scheduler interruption and canonical
Python launch/source reconciliation still gate live activation.
