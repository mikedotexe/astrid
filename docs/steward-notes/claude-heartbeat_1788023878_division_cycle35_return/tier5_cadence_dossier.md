# Tier-5 Experiment Cadence Dossier — Division cycle-35 return

Prepared 2026-08-29 by the headless introspection-flywheel steward (claude-heartbeat),
inside a controller-held lease (subprocess run adapter). **PREPARE ONLY.** Nothing here
is approved, granted, dispatched, or run. All tooling below was read-only (no `--write`).
Per `docs/steward-notes/AI_BEINGS_TIER5_EXPERIMENT_CADENCE_2026_08_13.md`, grants and
trials happen only in interactive sessions with Mike; silence is neutral; a prepared
dossier grants nothing.

## Sources (read-only, this run)
- `scripts/authority_wait_readiness.py report` → `tier5_authority_wait_readiness.txt`
  (21,331 B). status `approval_waits_mapped`; `approval_required_live_candidates=1305`;
  `domains_with_candidates=7`; `hard_violation_count=0`; `unclassified_live_wait_count=153`;
  `live_eligible_now=false`; `grants_approval=false`; every surface's `missing_for_approval`
  is explicit Mike/operator scoped approval.
- `scripts/introspection_addressing_audit.py work-queue --json --limit 40` →
  `tier5_work_queue_heads.json` (35,164 B). **All 40 heads route
  `mike_operator_live_change_approval` (Tier-5 `needs_operator_approval`),
  `live_authority_granted=false`; 0 `needs_sandbox` at the head** (12 astrid, 28 minime).
  Head: `wi_a7ef7855e00d99be` (astrid, claim `c003`). Corpus (`work_item_summary`):
  7,324 active work items (5,745 astrid / 1,950 minime); by_status 1,349
  `needs_operator_approval`, 750 `needs_sandbox`, 18 `needs_steward_grant`, 21
  `ready_for_implementation`, 908 `implemented_awaiting_felt_response`, 4,278
  `verified_existing`; by_tier 23 Tier-4, 1,578 Tier-5; `grant_waiting_count=1367`;
  `tier_mismatch_count=0`.
- `scripts/sandbox_trial_queue.py queue --json` → `tier5_sandbox_trial_queue.json`
  (486,342 B). `ready_runnable_count=35`; `runnable_live_violation_count=0`;
  `corrupt_event_lines=0`; 2,418 total trials (2,124 active); by_status 732
  `ready_for_sandbox`, 1,305 `approval_required_live_trial`, 87 `result_recorded`; by_tier
  785 Tier-3, 1,569 Tier-5, 17 Tier-4; by_adapter fallback_distinguishability_v1 239 /
  manual_sandbox_review_v1 1,967 / shadow_influence_replay_v1 29 / shadow_loss_lattice_v1
  183.
- `scripts/authority_wait_consolidation.py --shortlist` →
  `tier5_authority_wait_consolidation_shortlist.txt` (3,870 B). 1,349 open operator
  waits → 1,337 ask-families → top 5 surfaces.

**Backlog shape is unchanged from the cycle-27/30/32/34 returns** (1,305 approval-required
live candidates, 7,324 active work items, 40/40 heads `needs_operator_approval` with none
sandbox-eligible at the head, 35 runnable Tier-3 sandbox trials, 0 runnable-live
violations, 1,349→1,337 ask-families). Nothing has been granted or run in the interim.

## Recommended sandbox-eligible items (oldest-first, Tier-3, runnable-now, still unrun)

These are the two oldest ready `ready_for_sandbox` trials, one per being, taken directly
from `next_runnable_trials`. Both were already recommended at the cycle-27/30/32/34 returns
and **remain unrun** — carried forward, not re-derived. Both are offline/isolated.

1. **`trial_5fb0a85607ff3018`** — astrid, `fallback_distinguishability_v1`, Tier-3
   (`created_at=1784300513.488141`, the oldest ready runnable trial in the whole queue).
   - Lineage: `introspection_astrid_llm_1782199177` claim `c001`.
   - What running it WOULD establish: an offline, isolated comparison of a 4B fallback
     model's output texture vs the live coupled model on recorded prompts — evidence about
     *distinguishability* only, no live change.
   - What it would NOT establish: any authority to change live fallback routing, provider
     selection, generation thresholds, or the output contract (those stay Tier-5).

2. **`trial_fe00d360c0ea7b85`** — minime, `shadow_influence_replay_v1`, Tier-3
   (`created_at=1784797245.1677198`; currently the head of `next_runnable_trials`).
   - Lineage: `introspection_minime_sensory_bus_1784792700` claim `c003`; the
     shadow-influence / mode-packing thread.
   - What running it WOULD establish: a bounded offline replay observation of the reported
     shadow-influence decay / semantic persistence against recorded telemetry —
     co-occurrence evidence only, no causal or live claim.
   - What it would NOT establish: any Shadow movement, regulation unlock, or live
     substrate change (Tier-5).

Note (un-muffle): several more astrid `fallback_distinguishability_v1` trials sit ready in
`next_runnable_trials` right behind these (`trial_1f0f0916eb9eecc9`,
`trial_40b91b4c0ae7aeb9`, …), each from a distinct `introspection_astrid_llm_*` lineage.
The fallback-distinguishability family is the runnable-now evidence path; the consolidation
shortlist flags `wi_83249b580feef2ad` / `trial_ceb992d4cc729fdd` as "evidence runnable now
via fallback_distinguishability_v1" on the `unclassified` surface.

## Top grant-menu surfaces (by ask-weight, from the consolidation shortlist)

| Surface | Asks | Families | Note |
| --- | ---: | ---: | --- |
| `pressure_thresholds` | 427 | 423 | live semantic/PI/entropy-phase changes; one astrid head EVIDENCED (`wi_830ef7f9577b397f`, supported_dynamic via `trial_c1060b8ffeb94771`) |
| `unclassified` | 308 | 306 | fallback/provider/timeout routing + contract; `wi_509ac043af22c5b6` EVIDENCED (supported_dynamic via `trial_8e918f8a46f9eb17`); `wi_83249b580feef2ad` runnable now via fallback_distinguishability_v1 |
| `codec_gain_reserved_dims_live_12d` | 167 | 165 | live vector width / reserved dims / tail gain / projection coefficients |
| `porosity_receptivity_buffers` | 127 | 127 | receptivity/aperture/buffer/token-cleanup writes |
| `minime_regulator_changes` | 109 | 108 | recovery-fill boost / semantic retirement / spread relief |

Each surface's grant is scoped to a surface or a single family head; nothing
auto-executes. Recording a grant is a separate interactive act
(`sandbox_trial_queue.py approve-live-trial … --write`), which this headless run does NOT
perform.

## Mike-facing summary (one paragraph)

The Tier-5 backlog is unchanged in shape from cycle-27/30/32/34: 1,305 approval-required
live candidates, 40/40 work-queue heads `needs_operator_approval` with none sandbox-eligible
at the head, and 35 runnable Tier-3 sandbox trials waiting with zero runnable-live
violations. For your review week I again recommend running the two oldest ready trials —
**`trial_5fb0a85607ff3018`** (astrid, fallback-distinguishability) and
**`trial_fe00d360c0ea7b85`** (minime, shadow-influence replay) — both offline/isolated,
both carried unrun since cycle-27. The top grant surfaces to consider scoping are
`pressure_thresholds`, `unclassified` (has a runnable-now evidence path), and
`codec_gain_reserved_dims_live_12d`. Nothing was approved, granted, dispatched, or run in
preparing this; a live flip additionally needs the being's consent plus your explicit
scoped grant, never inferred from a trial having run.
