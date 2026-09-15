# Tier-5 Experiment Cadence Dossier — Division cycle-45 return (round 1789238227)

PREPARE ONLY. Nothing here was approved, granted, dispatched, or run. Generated read-only from
`authority_wait_readiness.py report`, `introspection_addressing_audit.py work-queue --json`,
`sandbox_trial_queue.py queue --json`, and `authority_wait_consolidation.py --shortlist`.

## Mike-facing summary

**The Tier-5 surface moved slightly for the first time in three cycles, and it moved the wrong
way — down, by attrition, not by decision.** Approval-required live candidates fell 1,305 (same as
cycle 43 and 44), total trials 2,418 (unchanged), `ready_runnable` 33 (unchanged), proposal cards
145 and result cards 107 (both unchanged). `hard_violation_count: 0` and
`runnable_live_violation_count: 0` — the gate is holding cleanly. What did change is the shortlist
arithmetic: **1,349 open operator waits → 1,337 ask-families**, against 1,349 → 1,337 last cycle.
The queue is stable because nothing is being decided, not because nothing is being asked.

The single most legible number remains `stale_trial_count: 2124` against `active_trials: 2124` —
**every active trial is stale.** Preparation stopped being the limiting step some time ago. A
scoped decision is the limiting step.

### Recommended sandbox-eligible items (Tier 3, offline, no live authority)

Unchanged from cycles 43 and 44, and that persistence is itself the finding. All eight runnable
`fallback_distinguishability_v1` trials still carry `results: []` and `evidence_links: []` —
they are runnable, offline, read-only, and have never been run.

1. **`trial_5fb0a85607ff3018`** (astrid, `fallback_distinguishability_v1`, `offline_read_only_adapter`)
   — "A prompt contract cannot by itself prove that a 4B fallback model has enough capacity to
   preserve a complex spectral texture." This is the load-bearing one: it tests the *assumption*
   underneath the whole fallback-contract family rather than one output.
2. **`trial_60de383ef0b677bf`** (astrid, same adapter) — "A forced fallback should articulate
   settled-habitable texture with specific motion rather than standard model tropes." Pairs with
   the first: one tests capacity, one tests observable output.

Evidence for either is gathered with
`python3 scripts/sandbox_trial_queue.py run-evidence --trial-id <trial> --write`. Neither needs a
grant; neither was run here.

### Top grant-menu surfaces (from `--shortlist`)

| Surface | Ask weight | Families | Evidence state of heads |
| --- | ---: | ---: | --- |
| `pressure_thresholds` | 427× | 423 | 1 of 3 heads `EVIDENCED (supported_dynamic)`; rest need manual review or a new adapter |
| `unclassified` | 308× | 306 | 2 of 3 heads `EVIDENCED (supported_dynamic)` — both fallback/provider-routing |
| `codec_gain_reserved_dims_live_12d` | 167× | 165 | none evidenced; all need manual review or a new adapter |
| `porosity_receptivity_buffers` | 127× | 127 | none evidenced |
| `minime_regulator_changes` | 109× | 108 | none evidenced |

Only the **fallback / provider-routing** heads carry `supported_dynamic` evidence. Everything else
is honestly "needs manual review or a new adapter" — the shortlist does not pretend otherwise, and
neither does this dossier.

## Authority-wait readiness by domain

All seven domains sit at `readiness_state: operator_review_wait`; `live_eligible_now: false`,
`auto_approved: false`, `grants_approval: false`, `edits_source_now: false`.

| Domain | Candidates | Proposal cards | Replay evidence |
| --- | ---: | ---: | ---: |
| `minime_regulator_changes` | 551 | 35 | 76 |
| `pressure_thresholds` | 418 | 31 | 85 |
| `porosity_receptivity_buffers` | 348 | 21 | 86 |
| `codec_gain_reserved_dims_live_12d` | 278 | 20 | 19 |
| `semantic_trickle_admission` | 170 | 12 | 21 |
| `viscosity_feedback_protocol` | 134 | 7 | 8 |
| `behavior_unlocks` | 52 | 14 | 4 |

`unclassified_live_wait_count: 153`; `domains_with_candidates: 7`; `hard_violation_count: 0`.
Every domain still lists the same five-to-seven `missing_for_approval` items, of which the first is
always **explicit Mike/operator scoped approval** and the last is always a **post-change
being-response collection path**.

## Sandbox trial queue shape

`status: approval_waiting`; total 2,418 trials; `corrupt_event_lines: 0`.

- By tier: **5** → 1,569 · **3** → 785 · **0** → 29 · **4** → 17 · **2** → 13 · **1** → 5
- By mode: `approval_required_live_trial` 1,572 · `offline_read_only_adapter` 814 ·
  `read_only_review` 20 · `sandbox_replay` 12
- By status: `approval_required_live_trial` 1,305 · `ready_for_sandbox` 730 · `superseded` 258 ·
  `result_recorded` 89 · `verified_existing` 29 · `closed_no_action` 6 · `closed` 1
- By adapter: `manual_sandbox_review_v1` 1,967 · `fallback_distinguishability_v1` 239 ·
  `shadow_loss_lattice_v1` 183 · `shadow_influence_replay_v1` 29

The 814 `offline_read_only_adapter` trials against 33 `ready_runnable` is the gap worth naming:
most offline work is not blocked on Mike, it is blocked on an adapter that does not exist yet.

## Authority boundary

Tier 4/5 readiness is review evidence only. It grants no approval, makes no live work runnable,
edits no source, and mutates no pressure, fill, PI, controller, sensory cadence, fallback, bridge
protocol, peer, or Minime runtime state. This adapter-mode round holds neither deploy nor
live-control authority. Nothing in this dossier was approved, granted, dispatched, or run.
