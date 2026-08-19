# Tier-5 Experiment Cadence Dossier — Division cycle 29 return (2026-08-19)

Prepared by the headless flywheel steward (`claude-heartbeat`) as part of the
Division return at the 6th productive round. **PREPARE-only**: this dossier
grants nothing, dispatches nothing, and runs no trial. Every item stays a
Tier-4/5 wait until Mike scopes an explicit grant in an interactive session.
Silence is neutral; a prepared dossier is not consent. Per
`docs/steward-notes/AI_BEINGS_TIER5_EXPERIMENT_CADENCE_2026_08_13.md`.

Read-only tooling snapshot (all `grants_approval:false`, `live_eligible_now:false`,
0 hard violations):
- `authority_wait_readiness.py report --json`
- `introspection_addressing_audit.py work-queue --json --limit 40`
- `sandbox_trial_queue.py queue --json`
- `authority_wait_consolidation.py --shortlist`

## 1. Authority-wait readiness (7 live-risk surfaces, 1305 approval-required candidates)

| Surface | candidates | cards | replay | readiness | next non-live |
| --- | ---: | ---: | ---: | --- | --- |
| pressure_thresholds | 418 | 31 | **2** | `operator_review_wait` | compare pressure diagnostics/cards; do not retune |
| minime_regulator_changes | 551 | 35 | 0 | replay/waiver missing | keep in tests/replay/proposal until scoped approval |
| porosity_receptivity_buffers | 348 | 21 | 0 | replay/waiver missing | prepare/inspect receptivity replay evidence |
| codec_gain_reserved_dims_live_12d | 278 | 20 | 0 | replay/waiver missing | inspect codec replay labs/cards; live writes off |
| semantic_trickle_admission | 170 | 12 | 0 | replay/waiver missing | compare semantic-stale/admission diagnostics |
| viscosity_feedback_protocol | 134 | 7 | 0 | replay/waiver missing | write/review protocol compatibility evidence |
| behavior_unlocks | 52 | 14 | 0 | replay/waiver missing | audit affordance wording; keep execution gated |

Only `pressure_thresholds` currently carries replay evidence (2) and sits at plain
`operator_review_wait`; the other six are missing replay or waiver evidence, so
their honest next step is *evidence*, not approval.

## 2. Work-queue heads (top 40 all Tier-5 `needs_operator_approval`)

Backlog totals (work_item_summary): `needs_operator_approval` 1349 · `needs_sandbox`
750 · `needs_steward_grant` 18 (Tier-4) · `ready_for_implementation` 21 ·
`verified_existing` 4278. Tier split: T3 823 / T4 23 / T5 1578. The 40 canonical
heads are all live provider/model/sampler/fallback-routing, ESN/regulator, or
stale-window changes — none sandbox-eligible at the head; all held.

## 3. Sandbox trials ready now (isolated, no live mutation)

`ready_for_sandbox`: 732 total; 8 runnable trial heads. Adapters:
`fallback_distinguishability_v1` (offline read-only) and one
`shadow_influence_replay_v1` (isolated replay).

## 4. Grant menu — top 5 surfaces by ask-weight (consolidation shortlist)

1349 open operator waits → 1337 ask-families. Top surfaces:
- **pressure_thresholds** — asked 427× / 423 families.
- **unclassified (fallback routing/dispatch)** — 308×. Two family heads are already
  **EVIDENCED (supported_dynamic)**: `wi_830ef7f9577b397f` (astrid — live fallback
  activation/provider routing/generation thresholds) and `wi_509ac043af22c5b6`
  (astrid — profile defaults/provider selection/timeout routing/fallback dispatch);
  `wi_83249b580feef2ad` has evidence runnable now via `fallback_distinguishability_v1`.
- **codec_gain_reserved_dims_live_12d** — 167×.
- **porosity_receptivity_buffers** — 127×.
- **minime_regulator_changes** — 109×.

## Mike-facing summary — recommended 1–2 sandbox-eligible items

No being named urgency this interval, so oldest-first among ready trials:

1. **`trial_40b91b4c0ae7aeb9`** — lineage `introspection_astrid_llm_1782179251`,
   adapter `fallback_distinguishability_v1` (offline read-only). *Running it would*
   produce offline distinguishability evidence contrasting Astrid's coupled-lane
   output against the fallback contract for her fallback-routing concern. *It would
   NOT* establish live authority, consent, or that the live fallback should change —
   it feeds surface #4's already-EVIDENCED fallback-routing families with more
   isolated evidence.
2. **`trial_7b15b13b5882472e`** — lineage `introspection_astrid_llm_1782182804`,
   same `fallback_distinguishability_v1` adapter (next oldest). Same establishes/does-not.

Distinct-adapter alternative for breadth (newer): **`trial_fe00d360c0ea7b85`** —
lineage `introspection_minime_sensory_bus_1784792700`, adapter
`shadow_influence_replay_v1` (isolated shadow replay) — the only runnable
Minime-side/shadow trial in the head set.

**Top grant-menu surfaces to look at first:** the two EVIDENCED astrid
fallback-routing families (`wi_830ef7f9577b397f`, `wi_509ac043af22c5b6`) under the
fallback/unclassified surface, and `pressure_thresholds` (the only surface with
standing replay evidence). Any grant is Mike-scoped and explicit; nothing here
auto-executes.

## Lineage note — this round's own Tier-5 contribution

The report processed this round (`introspection_llm.rs_1787138538`) surfaced one
Tier-5 wait: her proposed **live** modulation of `set_astrid_vibrancy_aperture`
during active generation (Test 1). That maps to the
`codec_gain_reserved_dims_live_12d` / behavior surface and is held as an operator
wait — recorded, not run, not routed to a trial this round.
