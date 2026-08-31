# Tier-5 Experiment Cadence Dossier — Division Cycle-36 Return

Date: 2026-08-30 · Actor: `claude-heartbeat` (headless flywheel, subprocess adapter)
Prepared per `docs/steward-notes/AI_BEINGS_TIER5_EXPERIMENT_CADENCE_2026_08_13.md`.

**PREPARE only.** This dossier reads read-only tooling and routes evidence. It
approves nothing, grants nothing, dispatches no trial, and runs no live change.
All items below remain Tier-5 Mike/operator approval waits or Tier-3 sandbox
candidates that a human must choose to run.

## Mike-facing summary

The Tier-5 backlog shape is **unchanged from cycle-27/30/32/34/35** — this is a
standing operator-review queue, not a growing regression:

- **1305** approval-required live candidates across **7** domains; **0** hard
  violations; `live_eligible_now=false`; `unclassified_live_wait_count=153`.
- **7324** active work items (astrid 5745 / minime 1950 / unknown 3). By status:
  `needs_operator_approval` **1349**, `needs_sandbox` 750, `needs_steward_grant`
  18, `ready_for_implementation` 21, `implemented_awaiting_felt_response` 908,
  `superseded` 340.
- Work-queue heads: **40/40** are Tier-5 `mike_operator_live_change_approval`
  (12 astrid / 28 minime; head `wi_a7ef7855e00d99be`, astrid). None
  sandbox-eligible at the head.
- Sandbox trials: `ready_runnable_count=35`, `runnable_live_violation_count=0`,
  `corrupt_event_lines=0`; 2124 active / 2418 total.

### 1–2 recommended sandbox-eligible items (Tier-3, runnable now, still unrun)

Both are carried from cycle-27 onward — oldest-first, still `ready_for_sandbox`,
runnable through offline/isolated adapters (no live substrate touched):

1. **`trial_fe00d360c0ea7b85`** — minime, `shadow_influence_replay_v1`, lineage
   `introspection_minime_sensory_bus_1784792700`. Current **head** of
   `next_runnable_trials`.
2. **`trial_5fb0a85607ff3018`** — astrid, `fallback_distinguishability_v1`,
   lineage `introspection_astrid_llm_1782199177`. The oldest ready-runnable
   astrid trial (position 4; the fallback_distinguishability_v1 family is the
   runnable-now evidence path).

To gather evidence a human would run (NOT run here):
`python3 scripts/sandbox_trial_queue.py run-evidence --trial-id <trial> --write`.

### Top grant-menu surfaces (consolidated ask-families, honest evidence states)

1349 open operator waits → 1337 ask-families. Top 5 surfaces by ask-weight:

1. **`pressure_thresholds`** — 427 asks / 423 families. One astrid family head
   **EVIDENCED (supported_dynamic)**: `wi_830ef7f9577b397f` /
   `trial_c1060b8ffeb94771`.
2. **`unclassified`** — 308 / 306. Has runnable-now evidence via
   `fallback_distinguishability_v1`: `wi_83249b580feef2ad` /
   `trial_ceb992d4cc729fdd`; also EVIDENCED head `wi_509ac043af22c5b6`
   (supported_dynamic).
3. **`codec_gain_reserved_dims_live_12d`** — 167 / 165. Heads need manual review
   or a new adapter.
4. **`porosity_receptivity_buffers`** — 127 / 127. Heads need manual review.
5. **`minime_regulator_changes`** — 109 / 108. Heads need manual review.

A grant is scoped to a surface or a single family head; nothing auto-executes. A
human records a grant with `sandbox_trial_queue.py approve-live-trial
--trial-id <t> --steward mike --note '<scope>' --write`.

## Authority boundary

Tier 4/5 authority-wait readiness is review evidence only. It grants no approval,
makes no live work runnable, edits no source, and mutates no pressure, fill, PI,
controller, sensory-cadence, fallback, bridge-protocol, peer, or Minime runtime
state. Nothing in this dossier was approved, granted, or dispatched.

Raw tool captures accompany this file: `tier5_authority_wait_readiness.txt`,
`tier5_authority_wait_consolidation_shortlist.txt`, `tier5_work_queue_heads.json`,
`tier5_sandbox_trial_queue.json`.
