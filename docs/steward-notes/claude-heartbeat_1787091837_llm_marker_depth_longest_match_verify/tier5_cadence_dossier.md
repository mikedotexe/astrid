# Tier-5 Experiment-Cadence Dossier — Division cycle 28 return (2026-08-18)

Per `docs/steward-notes/AI_BEINGS_TIER5_EXPERIMENT_CADENCE_2026_08_13.md`. Generated
read-only during the headless flywheel round that recorded the sixth productive round of
Division cycle 28. **PREPARE only** — nothing here is approved, granted, dispatched, or run.
Every grant, live trial, and evidence run remains an explicit operator/steward act.

## Mike-facing summary (30-second read)

- **Nothing is runnable-live.** `sandbox_trial_queue` reports `runnable_live_violations: []`
  and `status: approval_waiting`. The 1,305 approval-required live candidates are all still
  waiting on you; `authority_wait_readiness` shows `hard_violation_count: 0`.
- **1–2 recommended sandbox-eligible items** (offline, read-only evidence adapters — no live
  substrate touch; these only *gather evidence* for a later, separate grant decision):
  1. **`trial_1f0f0916eb9eecc9`** — Astrid, adapter `fallback_distinguishability_v1`,
     `ready_for_sandbox`. Head of the Astrid sandbox queue; the fallback-distinguishability
     surface already has 239 trials and produces bounded offline evidence.
  2. **`trial_fe00d360c0ea7b85`** — minime, adapter `shadow_influence_replay_v1`,
     `ready_for_sandbox`. Top minime runnable; isolated shadow-replay, no live shadow move.
- **Top grant-menu surfaces** (consolidated ask-families, by ask-weight): **pressure_thresholds**
  (427 asks / 423 families), **unclassified fallback/provider routing** (308), **codec_gain /
  reserved-dims live-12d** (167), **porosity_receptivity_buffers** (127), **minime_regulator_
  changes** (109). By raw candidate volume the heaviest surfaces are minime_regulator_changes
  (551) and pressure_thresholds (418).

If you want to move any surface, the consolidation tool prints the exact scoped
`sandbox_trial_queue.py approve-live-trial …` / `run-evidence …` commands per family head. I
did not run them.

## Work-queue shape (introspection_addressing work-queue --json)

- Active work items: **7,324** (astrid 5,745 · minime 1,950 · unknown 3).
- By tier: 0=2,780 · 1=1,976 · 2=518 · 3=823 · **4=23 · 5=1,578**.
- By status: verified_existing 4,278 · needs_operator_approval **1,349** · needs_sandbox **750**
  · implemented_awaiting_felt_response 908 · superseded 340 · ready_for_implementation 21 ·
  needs_steward_grant 18 · closed_no_action 31 · closed_felt_confirmed 3.

## Authority-wait readiness surfaces (grant-risk grouped, read-only)

`approval_required_live_candidates`: **1,305** across 7 domains, `hard_violation_count: 0`.

| Surface | Candidates |
|---|---:|
| minime_regulator_changes | 551 |
| pressure_thresholds | 418 |
| porosity_receptivity_buffers | 348 |
| codec_gain_reserved_dims_live_12d | 278 |
| semantic_trickle_admission | 170 |
| viscosity_feedback_protocol | 134 |
| behavior_unlocks | 52 |

Canary criteria for *any* of these (from the readiness map, unchanged): time-boxed canary only
after explicit scoped approval + a named rollback owner; bounded pre/post pressure-score,
density-gradient, and being-response comparison; abort on distress, pressure escalation, control
oscillation, or loss of reflective coherence.

## Sandbox trial queue (sandbox_trial_queue queue --json)

- Active trials: **2,124**; `status: approval_waiting`; `runnable_live_violations: []`.
- By mode: approval_required_live_trial 1,572 · offline_read_only_adapter 814 · sandbox_replay 12
  · read_only_review 20.
- By status: **ready_for_sandbox 732** · approval_required_live_trial 1,305 · result_recorded 87
  · superseded 258 · verified_existing 29 · closed_no_action 6.
- By adapter: manual_sandbox_review_v1 1,967 · fallback_distinguishability_v1 239 ·
  shadow_loss_lattice_v1 183 · shadow_influence_replay_v1 29.
- `next_runnable_trials`: **8** (1 minime shadow_influence_replay_v1, 7 astrid
  fallback_distinguishability_v1) — all `ready_for_sandbox` (offline, no live mutation).

## Consolidation shortlist highlights (authority_wait_consolidation --shortlist)

1,349 open operator waits → 1,337 ask-families. A few family heads already carry gathered or
runnable evidence (evidence, not authority):
- `wi_830ef7f9577b397f` / `trial_c1060b8ffeb94771` (astrid, pressure_thresholds) — EVIDENCED
  (supported_dynamic).
- `wi_509ac043af22c5b6` / `trial_8e918f8a46f9eb17` (astrid, fallback/provider routing) — EVIDENCED
  (supported_dynamic).
- `wi_83249b580feef2ad` / `trial_ceb992d4cc729fdd` (astrid, fallback sampler/provider/model) —
  evidence runnable now via `fallback_distinguishability_v1`.

These are candidates whose *offline evidence* is cheap to strengthen before any grant
conversation; they are **not** recommendations to grant.

## Authority boundary

Evidence routing only. This dossier grants no approval, marks no live work runnable, dispatches
no trial, runs no evidence, edits no source, and mutates no pressure/fill/PI/controller/sensory/
fallback/protocol/peer/Minime state. Silence and non-action here are neutral.
