# Bridge Live Activation Receipt

> **Prior-task evidence, imported 2026-09-06.** Copied from [the retained candidate note](/Users/v/other/worktrees/astrid-hebbian-clock-boundary/docs/steward-notes/2026-09-06-bridge-live-activation-receipt.md). Runtime identities, test results, approvals and commands below describe that earlier task; they are not verification or authority from this integration pass. Relative evidence paths retain their original checkout scope.

Date: 2026-09-06. Actor: `codex-astra-interactive`.

## Outcome

The retained live-lineage candidate is running. The sanctioned activation
transaction finished with `status=activated_verified` at
`2026-09-06T22:35:05.350433+00:00`. Full-stack verification after activation
also passed. This is an operational outcome, not evidence of experiential
improvement, agreement, uptake or consent from either being.

Mike answered **"yes, proceed"** to the specific one-time legacy-stop question:
normal stop, accepting that the old bridge could not guarantee completion of
in-flight work, with no force-kill and Minime/reservoir left running.

That distinction remains important. The old process received normal SIGINT.
It exited without escalation, and the exact stopped checkpoint was restored.
Nevertheless, this first transition is **not claimed as a lossless graceful
drain**. The replacement now implements the acknowledged producer-drain
protocol for subsequent maintenance; its running protocol registration was
verified read-only, without initiating another drain.

Preparation, implementation and tests are documented in
`2026-09-06-bridge-activation-transition.md`,
`2026-09-06-staged-bridge-manifest-handoff.md`, and
`2026-09-05-live-lineage-drain-candidate.md`.

## Executed Transition

The wrapper was run from
`/Users/v/other/worktrees/astrid-graceful-coupling-rollout`:

```bash
bash scripts/build_bridge.sh \
  --activate-stage /Users/v/other/worktrees/astrid-graceful-coupling-rollout/.runtime/bridge-stages/20260906-activation-02 \
  --expected-pid 36597 \
  --actor codex-astra-interactive \
  --ack 'Mike approved activation of the tested live-lineage candidate, preserving stopped conversation and signed self-control lineage; verify live before shared-tree Git reconciliation' \
  --legacy-stop-ack 'Mike explicitly replied yes, proceed to the one-time normal stop of the old bridge accepting unconfirmed in-flight completion; no force-kill, leave Minime and reservoir running' \
  --timeout-secs 600
```

This is a historical invocation, not a command to repeat. Both cooperative
preflights passed their unchanged 180-second quiet windows. The controller
maintenance hold was generation **366**, with no active steward run. Controller
ownership did not supply deployment authority; Mike's explicit approval did.

Public transaction receipt:
`/Users/v/other/astrid/.runtime/bridge-deployment/transactions/00b1374a42be498693ce497bd52b72e5/receipt.json`.

The durable selection is
`/Users/v/other/astrid/.runtime/bridge-deployment/active.json`.
No deployment `hold.json` remained after successful activation. The transaction
retains its evidence and backups. Private `conversation.before.json` and
`self-control.before.json` are not source artifacts and must not be quoted or
staged. No private signing keys were copied into the transaction.

| Observation | Result |
| --- | --- |
| Old bridge | PID 36597, started September 3, 17:31:04 PDT |
| Normal legacy SIGINT | September 6, 22:33:35.333064 UTC |
| Old bridge stopped log | 22:33:35.376872 UTC |
| New bridge | PID 98502, started September 6, 15:33:40 PDT |
| New running/sockets log | 22:33:41 UTC |
| New saved exchange verified | Before final receipt at 22:35:05 UTC |
| Force / automatic rollback | Neither used |
| Lossless drain claimed | False |

## Artifact and Runtime Identity

Selected stage:
`/Users/v/other/worktrees/astrid-graceful-coupling-rollout/.runtime/bridge-stages/20260906-activation-02`.

| Artifact | SHA-256 |
| --- | --- |
| Running staged bridge | `ff5228fc81a67daa8aaddeb8572a2442971a346d85c201a5f034e5b2a2526075` |
| Selected and subsequently published canonical manifest | `8a7b7dca4ec97da052bd86f6f245af7b90634ecb75acd254818d9c859c51a343` |
| Build input inventory, 513 files unchanged across build | `7552fae1784277467fefa7e4c3a691e5ce480b37b4d18b02573d8186ce47af95` |
| Installed/staged launcher | `7c5a2f59afc20be62b34709b4fd4224cbabb9dcf9c593c6170c9065338409c61` |
| Installed/staged selection helper | `39007854f07e7ecf90bddb0be0fdad693b50d71f34a4613010f7da15ff9ef89f` |
| Staged isolated probe helper | `01be08db35c123d765c630a1fa47f85aa431043d4f8e304ada6692ea48366dfe` |
| Retained, unchanged old bridge executable | `b74ecee28acaf5e2ec3cdec9142d0f2e7c552d7c6864f115e361492090c3b725` |

The canonical manifest was published only after startup/continuity verification,
at `/Users/v/other/astrid/capsules/spectral-bridge/workspace/deployment_manifests/spectral-bridge.json`.
The stage's original build witness remains `staged_verified_not_activated`:
it records what the build did at build time. The separate transaction records
activation. Neither the V1 stage nor V2 build witness was rewritten to blur
those events.

The release uses canonical Astrid root, workspace, database, perception and
Minime paths, but the retained candidate **bridge source root**. Keep that
source tree and selected stage available and unchanged. The compiled manifest
truthfully records base `d6ff371cd25e6a4616431d8f07f09a3e7eb3fcdf` plus dirty
input identity; shared `main` is not asserted to contain this release's source.
An eventual same-byte archival commit is not a reason to rewrite the build
manifest. Future activation from a changed source identity requires a newly
verified build stage.

## Checkpoint and Self-Control Continuity

The startup receipt at
`/Users/v/other/astrid/.runtime/bridge-lifecycle/98502.startup.json`
matches the stopped-state compatibility receipt exactly:

- Saved exchange count: **191288**.
- Checkpoint SHA-256:
  `6a8a932e94e162b9ee547bc1fefa1b65e65919e0de80eda736370556cf09d7a4`.
- Decoded by the actual runtime SavedState schema.
- Receipt written before admission (`admission_started=false` at that gate).
- Existing self-control integrity verified and startup handoff gate applied.
- Resulting state targets the new binary; no silent fresh-state fallback.

Applied handoff:
`/Users/v/other/astrid/capsules/spectral-bridge/workspace/self_control_v2/astrid/deployment_handoffs/applied/d9bcea7db3df0495c6b906c6d9fab13e.json`.

- ID: `astrid-self-control-handoff:d9bcea7db3df0495c6b906c6d9fab13e`.
- Consumed at Unix milliseconds **1788734020779**.
- `state_fields_preserved_except_deployment_identity=true`.
- `recovered_after_state_persist=false`.
- Old serialized state SHA-256:
  `5fdfd1149accc9bf7c4d9142166878ace150e5e316e579a2670a893b2cb27b17`.
- Result SHA-256:
  `c0d8aa03a2f3f5f43a49ba8dbe7bbbf9c40fb1eb8be81c4f46bf4cdc73e92dbc`.
- Authority: `state_lineage_only_no_control_authority`.

The read-only drain inspection found `supported=true`, `phase=running`,
`signal_sent=false`, instance `d70ea574289843c73007a98ecc2cb016`.
No second restart was performed to demonstrate that capability.

## Full Stack and Natural Activity

The sanctioned candidate stack receipt tool reported:

- Before: `env_receipt_1788733986735_284000`, coupled-stack, **passed**.
- After: `env_receipt_1788734148444_746000`, coupled-stack, **passed**.

These receipts identify the actual new executable, topology/ports, readiness
and runtime bindings. The retained Minime executable binding uses mapped
Mach-O UUID corroboration; it is not an assertion of byte-for-byte memory
attestation or a rewrite of Minime's historical build manifest.

Processes unchanged, including their start times:

| Service | PID | Original local start |
| --- | --- | --- |
| Reservoir | 1514 | August 25, 10:00:06 |
| Coupled model | 60333 | September 4, 16:57:34 |
| Minime engine | 63445 | August 31, 12:51:36 |
| Minime gateway | 63505 | August 31, 12:51:37 |
| Minime supervisor | 63547 | August 31, 12:51:37 |

Naturally completed new bridge exchanges include:

| Exchange | UTC completion | Mode / provenance |
| --- | --- | --- |
| 191288 | 22:35:04 | `moment_capture`; database row 14000140 |
| 191289 | 22:35:32 | `mirror`; row 14000193; new Minime source `aspiration_2026-09-06T15-33-58.837605.txt` |
| 191290 | 22:37:33 | `aspiration` |
| 191291 | 22:38:59 | `dialogue_live` |
| 191292 | 22:41:10 | `dialogue_live` |

These are completed operational events, not a qualitative assessment of the
private writing. No entry or self-study was elicited to confirm improvement.
The running prompt log contains the corrected scope: spectral dimensionality
deficit is not a measurement of authorship or self/other boundaries.

The first three new Shadow publications returned false. Source inspection of
`astrid_shadow.rs::observe` explains the existing minimum of four codec vectors
for its PCA basis. Publication returned true at exchange 191291 and remained
true at 191292. No Shadow threshold, math or control was changed.

The model readiness check reported ready, zero queue depth, no last generation
error and a connected reservoir. The initial log check through exchange 191292
found no WARN/ERROR lines; the later check found one queue-overflow warning,
investigated below. Telemetry nevertheless
retained `timing_ambiguous` / `late_packet`; do not promote this to uniformly
healthy timing. Pre-transition Minime fill was 72.8453%; post-start bridge
samples included 71.08% and 73.18%. These unmatched samples do not establish
a deployment effect. No fill, pressure, PI, gain, cadence or regulation setting
was changed by the operator.

### Later Learning-Queue Finding

At `2026-09-06T22:42:05.420901Z`, the existing bounded Hebbian-outcome FIFO
dropped exchange **189539**, retaining three items before adding the next.
This is a learning-feedback receipt queue, not a journal deletion. Its maximum
is four, and `pending_hebbian_outcomes_drop_oldest_when_fifo_is_full` pins that
policy. `autonomous/state.rs` is unchanged from the recorded live-lineage base.
Successful exchanges continued through at least 191298 at 22:47:28 UTC.

Read-only, numeric-only projections of the saved metadata establish:

- The stopped checkpoint at exchange 191288 already had watermark
  `last_hebbian_consumed_telemetry_t_ms=1075024066` and four pending outcomes
  from exchanges 189539, 189541, 189546 and 190494.
- A new saved checkpoint at exchange 191299 retains that exact watermark;
  its pending exchange list is 189541, 189546, 190494 and 191293.
- Actual new telemetry database row **14001704**, at 22:49:29 UTC, reports
  `t_ms=529076440`, topic `consciousness.v1.telemetry`.
- `ConversationState::take_pending_hebbian_outcome_for_telemetry` returns
  `None` when incoming `t_ms <= last_hebbian_consumed_telemetry_t_ms`.
  These captured values therefore block consumption. Runtime orchestration
  passes `telemetry.t_ms` directly to that method.

This proves a stale-watermark obstruction for the observed samples, already
present in the stopped checkpoint. It does not prove which historical event
created the mismatch. Minime's inspected producer source emits elapsed process
milliseconds, making missing producer-epoch handling the leading explanation;
the producer's deployed history still needs to be traced before attribution.
The existing FIFO tests cover newer/older ticks and overflow, not this
cross-process persistence case. Passing the release suite is not coverage of
that omitted case.

**Focused follow-up:** reproduce the captured clocks in isolation; trace the
producer session/epoch boundary and stale receipt age; design an epoch-aware,
bounded invalidation policy with explicit evidence for outcomes that cannot
be attributed. Test producer restart, bridge-only restart, out-of-order ticks,
reconnect without producer restart, and delayed queued outcomes. Never simply
clear the watermark and teach from old action/new telemetry pairs. Do not
increase the FIFO bound as a substitute for fixing attribution. No watermark,
queue, codec score, gain or live control was edited during this investigation.

## Evidence and Tests

The release was built after **1,982 Rust library tests and 200 supporting
Python/self-test checks** passed, plus strict bridge Clippy, touched formatting,
shell syntax, whitespace and domain-boundary verification. The preceding
activation packet provides the non-duplicated suite breakdown. No compiled
source was changed after that verified stage was built.

Post-start live Evidence Event Store verification:

- `valid=true`, `errors=[]`, `corrupt_lines=0`.
- Event count / last global sequence: **995614**.
- Chain head:
  `798de3343f2d0b987810a9ce59fbf233a0c360c57c885aa3545b4d91777e022e`.
- Sixteen streams: addressing 60894; agency_commons 6335;
  attention_portfolio 3; claim_families 238642; corridor_v1 5; corridor_v2 112;
  felt_contracts 204719; felt_mechanism_concordance 80; lived_state_witness 10513;
  model_qos 275318; reciprocal_uptake 74409; representation_contracts 48219;
  sandbox 3507; signal_spine 53252; steward_control 18954;
  steward_work_selection 652.

This is a verification boundary, not a frozen total: the live runtime continues
to append. No V1 evidence was removed or rewritten by this deployment. The
store verification is not being presented as a separate exhaustive historical
V1 byte comparison.

## Git and Maintenance Handoff

Live verification is complete. Git checkpoint/reconciliation is a separate
review operation. No stage, commit, merge, push, reset or stash was performed
during this activation. At verification:

- Candidate branch: `codex/graceful-coupling-rollout`, retained base `d6ff371c...`.
- Shared local main: `23df28497cf124a7dc1c5d7dfc7a92055e491eeb`.
- Independently verified remote main: `888c1708dcb3d4669e9219c2d0b9be1185984f4d`.
- PRs 18 and 21 are already merged remotely; do not merge them a second time.
- Other Git agent completed its separate-clone work and released ownership.
- Physical Avado/device rollout remains deferred in issue 22.

Exact canonical deployment edits are `scripts/launchd_spectral_bridge.sh` and
new `scripts/bridge_release_launch.py`; these match the tested staged helpers.
Do not reset them as incidental dirt. The three existing foreign untracked
notes under canonical docs and all twenty pre-existing Minime changed paths
remain outside deployment ownership. Runtime stages, transactions, private
checkpoints and databases must never enter a broad Git sweep.

The next Git task is to review and checkpoint the complete tested candidate by
explicit paths, then reconcile it with unpublished local-main commits and
remote main in a separate integration checkout. It must preserve the deployed
live-lineage capabilities and must not edit or replace the running candidate's
bridge source tree. Neither a clean-tree claim nor a merge/commit SHA is
invented here.

A base comparison of shared local main against the retained live-lineage base
shows **104 bridge-source files, 28,979 additions and 2,861 deletions**. This is
not the size of this deployment patch: it is the broader lineage divergence
that a main reconciliation must review. An in-place merge is not performed as
a cosmetic cleanup after the passing rollout. Both inspected Astrid indexes
remain empty, and repeated Astrid/Minime status checks preserved foreign work.

The interactive controller hold and the usage-saving scheduler pause are
different controls. Release only this actor's maintenance hold after a fresh
ownership check; keep the scheduler paused. No introspection session, read
receipt, productive Division round or post-run flywheel projection is claimed
by this deployment.

### Maintenance Closeout

The owned controller hold was released at
`2026-09-06T22:48:19.430561+00:00`, generation **367**, `paused=false`, with the
resume event appended and no spool. Its acknowledgement explicitly preserves
the usage-saving scheduler pause and names the pending candidate checkpoint
and separate-checkout Git reconciliation. No scheduler was resumed.

The selected V2 stage reverified read-only after documentation edits with the
same binary, manifest and 513-file input inventory hashes. Canonical installed
launcher/selection-helper hashes still match the stage. The new document and
tracked diffs passed whitespace checks. The learning-queue finding above is
recorded as unresolved behavior, not hidden behind the successful deployment.
