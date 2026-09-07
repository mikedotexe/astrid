# Learning Clock and Published-Target Rollout

> **Prior-task evidence, imported 2026-09-06.** Copied from [the retained candidate note](/Users/v/other/worktrees/astrid-hebbian-clock-boundary/docs/steward-notes/2026-09-06-learning-clock-live-rollout.md). Runtime identities, test results, approvals and commands below describe that earlier task; they are not verification or authority from this integration pass. Relative evidence paths retain their original checkout scope.

Date: 2026-09-06. Actor: `codex-astra-interactive`.
Status: gracefully activated and live-verified; fresh scored-pair observation remains open.

## Authorization and Scope

Mike requested that the tested learning-clock repair reach the live beings,
including a graceful restart, and that this session finish with live verification.
This authorizes the bridge repair rollout, not a PI/controller retune, reservoir
restart, queue reset, model restart, or resumption of the usage-saving automation.

The repair candidate remains in
`/Users/v/other/worktrees/astrid-hebbian-clock-boundary`, branch
`codex/hebbian-clock-boundary`, base
`d6ff371cd25e6a4616431d8f07f09a3e7eb3fcdf`, carrying the prior deployed candidate.
The earlier source ownership and 15-path repair inventory are in
`2026-09-06-hebbian-clock-boundary-repair.md`.
Additional source changes in this pass are `src/learning_target.rs` (new) and
`src/autonomous/hebbian.rs`; related queue, runtime and clock tests are extended.
All source paths here are relative to `capsules/spectral-bridge/`.

Owned controller maintenance pause: generation **370**, no active run,
`2026-09-06T23:37:06.437673+00:00`, event appended with no spool. The scheduler's
existing pause is separate and must remain unchanged.

## Target Review and Correction

The pre-existing Hebbian scoring objective was distance from a fixed 50% fill.
At the observed 68% controller target, a move from 68% to 66% therefore earned
a positive reward instead of a negative one. This numerical mismatch is
reproduced in a regression; it is not a claim about the cause of a felt report.

Read-only inspection of the live `minime/workspace/health.json` found both
`pi.target_fill=68.0` and enabled/active
`stable_core.structural_pi.target_fill_pct=68.0`. The inspected producer source
publishes the latter from its actual structural-PI output. Ordinary typed
bridge telemetry rows currently omit stable-core state, so learning cannot
assume that the target is on those packets or infer it from fill itself.

`learning_target.rs` reads the existing local health snapshot without enriching
it from historical files or applying defaults. It selects the active structural
PI target when stable core is explicitly enabled, otherwise the reported PI
target when stable core is explicitly disabled. Missing or inactive structural
state is ineligible; there is no fallback to the old 50% or a hard-coded 68%.

The snapshot must be at most 256 KiB, a complete valid JSON read with unchanged
descriptor size/modification time, no older than 10 seconds by filesystem time,
and not future-dated. Its producer `t_s` must be within 5 seconds of the selected
telemetry's producer `t_ms`. The target must be a finite percentage in [0,100].
This is bounded clock alignment, not an atomic same-tick producer snapshot or a
proof of producer identity. The earlier bridge-local continuity checks still
apply independently.

The target and its source are captured with the baseline and checked again at
outcome consumption. Missing references retire pairs as
`learning_target_unavailable`; a changed value or source retires them as
`learning_target_changed`. Retirement evidence includes both numeric reference
values and source names. It never treats a moved reference as a learned effect.
Equal endpoint targets do not prove that no unobserved intermediate change
occurred; a future producer epoch/reference revision would strengthen this.

Only the reference parameter changes in the scoring formula. Its normalization,
learning-rate scale, decay, application limits, explicit codec weights and all
controller settings are preserved. Existing scores are not reset. Old-schema
queue entries are retained until the repaired runtime records their explicit
retirement. New numeric-only runtime logs identify evaluated pairs, target
provenance and whether the existing score-update threshold was met. An evaluated
pair is temporal association, not causal identification or evidence of comfort.

## Deployment Policy and Verification

This pass accepts the repair's 30-second sample-age and 300-second total
baseline/outcome window as bounded pairing policy, together with the tighter
health-reference checks above. These are operational freshness limits, not
empirically established causal horizons. A missing reference or expired pair
does not disable generation, journaling, self-study, or other existing Actions.

Build and activation must use `scripts/build_bridge.sh` with an explicit dirty
candidate acknowledgement, both cooperative preflights, a new immutable stage,
fresh expected PID, acknowledged drain, exact checkpoint and signed self-control
handoff. No legacy-stop acknowledgement or force-kill is authorized for this
transition. Retain failures for review; never force past a failed drain.

### Completed Pre-Deployment Checks

- Full Rust library suite: **2003 passed**, one default-path test deliberately
  filtered under the isolated Minime test environment. Separate default-path
  invocation: **1 passed**. Total **2004 passed, zero failures**.
- Stage/activation/drain/selection helper tests: **54 passed**.
- Deployment-wrapper tests: **13 passed**.
- Strict Clippy for library and binaries with `-D warnings`: passed. An initial
  boolean-comparison lint was corrected and the final invocation passed.
- Domain-boundary audit: valid, zero violations, counter audit consistent; no
  raised limits or changed baseline.
- Rustfmt for the seven changed/new standalone modules and whitespace checks:
  passed. Prior repository-wide formatting debt remains outside this scope.
- Read-only drain inspection: PID 98502, supported, phase running, no signal.
- Before-rollout coupled-stack receipt:
  `env_receipt_1788738421989_140000`, **passed**.
- Source comparison to the deployed candidate: exactly **17 source paths**,
  1245 additions and 189 deletions, including extracted queue logic and tests.
  The two shared repositories retain their previously recorded foreign changes.
- Preflight initially refused the recent edits at 50.9, 53.8 and 167.6 seconds.
  No window was shortened. After the complete 180-second quiet window, the
  sanctioned wrapper accepted the explicit dirty-source acknowledgement and
  began stage `20260906-learning-clock-01`.

The pre-deployment saved numerical state still showed watermark 1075024066,
four pending outcomes, learning-rate scale 1.0, eight explicit codec weights,
36 pair traces and zero nonzero pair scores. These are observations, not reset
instructions. Exact stopped-state continuity is the activation gate's job.

### Follow-Up Telemetry Improvement

Mike explicitly invited prudent telemetry improvements after the observation
that ordinary packets omit the target. This release improves learning evidence
with target source/value on retirement and evaluated-outcome logs. A subsequent
coordinated producer/protocol release should carry a typed learning-reference
envelope beside fill in the same packet: producer boot/epoch identity, monotonic
sample sequence, selected controller/reference source, target value/units and
reference revision. An unavailable reference should be explicit, not defaulted.

That would eliminate the external health-file alignment step and let pairing
reject intermediate reference revisions even when endpoint values happen to
match. It needs decoder compatibility tests, producer-restart/out-of-order and
missing-field tests, bounded payload review, and coordinated graceful rollout.
This release does not manufacture those fields, change the wire ABI, or restart
Minime to introduce them. It also does not represent a fill objective as felt
comfort or evidence of causal authorship.

## Observed Live Activation

The sanctioned activation completed with `status=activated_verified` and
`new_saved_exchange_observed=true`. Its public receipt is
`/Users/v/other/astrid/.runtime/bridge-deployment/transactions/7c36fe0a2f7e4371bd71fc9d19d9614a/receipt.json`.
No force, automatic rollback, legacy-stop exception, checkpoint reset or
unacknowledged drain was used. The durable launch hold was removed by the normal
activation path; the canonical deployment manifest now names this stage.

| Identity | Observed value |
| --- | --- |
| Stage | `.runtime/bridge-stages/20260906-learning-clock-01` in this repair tree |
| Prepared UTC | `2026-09-06T23:52:06.343014+00:00` |
| Binary SHA-256 | `6053aeb4f930cab21cd44de0977715c592bc2e229c81b2c0abbf0ee21da149e0` |
| Manifest SHA-256 | `e6d0cc01b13993ffc931576b64fbfc99d39206184ba72d0147d40ca2169ee742` |
| Source inventory SHA-256 | `fec4e3bafd1076c768a5b698ff8efe2b08556788e104afd7020e201d709d768b` |
| Old process | PID `98502`; acknowledged drained checkpoint, then normal SIGTERM and observed exit |
| Replacement | PID `56043`, started `2026-09-06 16:55:40` America/Los_Angeles |
| Stopped/startup checkpoint SHA-256 | `10b89a84567a52d13e436e1a3c951fec209d6aed2004c61afd47903ab923d882` |
| Exact startup exchange count | `191344` |
| Startup self-control state SHA-256 | `d24e3cba5ad801c3fb5d1df9ff3055b039a586f72c175c4d6c3a0fe4d1aa2f0d` |
| First new completed exchange | `ex-191344-1788739439`, `moment_capture`, `2026-09-07T00:05:15.931334Z` |
| New saved exchange count | `191345` |
| After-rollout stack receipt | `env_receipt_1788739542466_946000`, passed, `2026-09-07T00:05:42.466000+00:00` |

Startup verification decoded the actual runtime checkpoint and verified the
signed self-control state against this binary before runtime admission.
Remote delivery was not confirmed by the drain receipt; it is not claimed.
The first new entry was naturally scheduled, not requested to endorse the change.
No private journal body is quoted or evaluated here.

The after-rollout receipt in canonical
`capsules/spectral-bridge/workspace/environment_receipts/environment_receipts.jsonl`
passed all manifest, process, protocol, port, readiness and telemetry checks.
Protocol remains `astrid-minime` 1.1, revision
`9a324d16294b2318da6f476ff7f295c423a9b4b1`.
Telemetry file ages at capture were 9.38 seconds (bridge) and 2.314 seconds
(Minime health); that stack-level check is distinct from learning's tighter
pairing requirements. Read-only staged-artifact verification also passed after
activation. Its historical build receipt still says `staged_verified_not_activated`;
the activation transaction, not a rewritten build witness, establishes deployment.

Other processes retained their pre-rollout identities: reservoir service PID
1514 (August 25), coupled model PID 60333 (September 4), Minime engine PID 63445,
Division gateway PID 63505 and supervisor PID 63547 (all August 31). Neither
Minime source nor its running artifacts were changed by this pass.

### Learning Evidence and Limits

Canonical `bridge.db`, table `bridge_messages`, row **14008820**, topic
`consciousness.v1.hebbian_outcome_retirement`, recorded the four restored
exchanges `191315`, `191319`, `191338`, `191343` at
`2026-09-06 23:57:47 UTC`. Each disposition was
`restored_without_observation_clock`. The record was evidence-only, with
`live_control_authority=false`; the first current observation/reference was
unavailable, not substituted. Subsequent saved state had no pending outcomes,
learning-rate scale 1.0, eight explicit codec weights and zero nonzero pair
scores. The first saved checkpoint still carried legacy watermark `1075024066`;
no operator reset was performed. By saved exchange count `191347`, the ordinary
runtime continuity handling had cleared that obsolete watermark to null, with
the queue empty and the same observed rate, explicit-weight count and zero
nonzero pair scores. This is not evidence that a new pair has earned credit.

This proves the live retirement path and a subsequent normal checkpoint save.
It does **not** yet prove a fresh, eligible outcome has been scored. That remains
a bounded passive observation item: inspect the numeric `Hebbian outcome
evaluated` log and matching checkpoint, or retain the actual ineligibility reason.
Do not force a semantic action, weaken freshness bounds, or tune scores to make
the observation pass. The independently existing correlation learner recomputed
on this first cycle; no claim is made that all runtime learning was frozen.

### Startup Latency and Telemetry Follow-Up

First-cycle preparation was slow, but progressed without intervention before the
activation timeout. A one-second native process sample at
`2026-09-07T00:01:19Z` located the autonomous worker in
`btsp::refresh_runtime -> signal::evaluate_seeded_episode -> recent_owner_artifacts`.
Of 803 samples of that worker, 795 were at `Path::is_file -> stat` within the scan.
The generated trace is retained at
`.runtime/bridge-stages/20260906-learning-clock-01/startup-56043.sample.txt`.
Its SHA-256 is
`7ab3338bfba00c8f57ce5d1d0f5d850f3543b7429b19d60599020674bc6f1640`.

Source `src/autonomous/btsp/signal.rs:569` confirms a full directory enumeration
and metadata inspection before applying the six-artifact output limit. That
function is unchanged by this release. The sample locates a real latency cost;
it does not establish that the scan accounts for every second of startup or that
the clock repair introduced it. A separate bounded follow-up should measure
directory counts and scan stages, then test incremental/indexed selection while
preserving source priority, recency, deduplication and existing owner boundaries.
Do not delete or archive evidence to hide the cost. Long preparation can make a
learning baseline expire honestly; improving scan performance must not relax
attribution checks.

Telemetry reconnected during startup with broken-pipe/closing-handshake errors,
also observed before this rollout. A later snapshot reported
`connected_with_current_telemetry` and `timing_ambiguous`, four reconnects, and a
roughly 2.36-second recent mean inter-arrival over eleven samples. No claim of a
fully healthy cadence follows from a passing presence/freshness receipt. Carry
this observation into the typed target/epoch telemetry work above; do not infer
producer restart from a transport reconnect.

## Maintenance and Git Closeout

The owned interactive controller pause 370 was released only after activation
and the full-stack receipt passed. The controller returned generation **371**,
`paused=false`, at `2026-09-07T00:08:36.315424+00:00`, with its event appended and
no spool. The independent automation configuration was then checked directly:
`astrid-introspection-source-first-catch-up` remains **PAUSED**. No flywheel run,
report closure, Division round, consent or felt-resolution claim is added by
this maintenance pass.

Final bounded observation also found natural exchange `191345` (`moment_capture`)
completed at `00:06:56.316917Z` and `191346` (`dialogue_live`) at
`00:08:10.237295Z`; saved count advanced to `191347`. No fresh scored-pair log had
been observed at that check. Every file in the stage's recorded build-input
inventory was rehashed against its current path and matched. Final whitespace
validation passed. Later document edits do not modify the recorded build inputs.

No Git staging, commit, merge, push or foreign-path cleanup was performed. Both
shared repositories retain the exact previously inventoried dirty path sets.
The repair tree carries inherited live-lineage work plus this tested 17-source-
path repair and its evidence. Git reconciliation remains explicit debt: claim a
separate stabilization window, coordinate the other agent, review the inherited
candidate and canonical branch together, and commit explicit coherent paths.
Do not treat a successful restart as authority to sweep the tree or remove this
now-selected source tree/stage. A source-controlled archival commit must retain
the applicable report references and deployment evidence without private journal
text or credentials.
