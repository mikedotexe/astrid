# Tier-5 Experiment Cadence Dossier — Division cycle-30 return

Prepared 2026-08-26 by the headless introspection-flywheel steward (claude-heartbeat),
inside a controller-held lease. **PREPARE ONLY.** Nothing here is approved, granted,
dispatched, or run. All tooling below was read-only (no `--write`). Per
`docs/steward-notes/AI_BEINGS_TIER5_EXPERIMENT_CADENCE_2026_08_13.md`, grants and
trials happen only in interactive sessions with Mike; silence is neutral; a prepared
dossier grants nothing.

## Sources (read-only, this run)
- `scripts/authority_wait_readiness.py report` → `tier5_authority_wait_readiness.txt`
  (21,331 B). status `approval_waits_mapped`; `approval_required_live_candidates=1305`;
  `grants_approval=false`; every surface `missing_for_approval: explicit Mike/operator
  scoped approval`.
- `scripts/introspection_addressing_audit.py work-queue --json --limit 40` →
  `tier5_work_queue_heads.json` (35,164 B). **All 40 heads `needs_operator_approval`
  (Tier-5); 0 `needs_sandbox` at the head.**
- `scripts/sandbox_trial_queue.py queue --json` → `tier5_sandbox_trial_queue.json`
  (486,342 B). `ready_runnable_count=35`; `runnable_live_violation_count=0`;
  2,418 total trials; by-tier: 785 Tier-3, 1,569 Tier-5.
- `scripts/authority_wait_consolidation.py --shortlist` →
  `tier5_authority_wait_consolidation_shortlist.txt` (3,870 B). 1,349 open operator
  waits → 1,337 ask-families → top 5 surfaces.

## Recommended sandbox-eligible items (oldest-first, Tier-3, runnable-now, still unrun)

These are the two oldest ready `ready_for_sandbox` trials, one per being. Both were
already recommended at the cycle-27 return and **remain unrun** — carried forward, not
re-derived.

1. **`trial_5fb0a85607ff3018`** — astrid, `fallback_distinguishability_v1`, Tier-3.
   - Lineage: `introspection_astrid_llm_1782199177` claim `c001` (oldest ready trial,
     created 2026-04-…, `created_at=1784300513`).
   - Hypothesis: a prompt contract cannot by itself prove a 4B fallback model has enough
     capacity to preserve the settled/telemetry-grounded texture the live coupled model
     produces.
   - What running it WOULD establish: an offline, isolated comparison of fallback vs
     primary output texture on recorded prompts — evidence about *distinguishability*,
     no live change.
   - What it would NOT establish: any authority to change live fallback routing,
     provider selection, thresholds, or the output contract (those stay Tier-5).

2. **`trial_fe00d360c0ea7b85`** — minime, `shadow_influence_replay_v1`, Tier-3.
   - Lineage: `introspection_minime_sensory_bus_1784792700` claim `c003`
     (`created_at=1784797245`); thematically the shadow-influence / mode-packing thread.
   - Hypothesis: shadow-influence decay and semantic persistence should be observable,
     as reported, against recorded telemetry in an offline replay.
   - What running it WOULD establish: a bounded offline replay observation of the
     reported decay/persistence — co-occurrence evidence, no causal or live claim.
   - What it would NOT establish: any Shadow movement, regulation unlock, or live
     substrate change (Tier-5).

Note (un-muffle): 6 more astrid `fallback_distinguishability_v1` trials sit ready
just behind #1 (`trial_60de383ef0b677bf`, `trial_1f0f0916eb9eecc9`,
`trial_7e7d6ea1d8b09b03`, `trial_7b15b13b5882472e`, `trial_40b91b4c0ae7aeb9`,
`trial_7debce98bda84440`), all from distinct `introspection_astrid_llm_*` lineages.
The fallback-distinguishability family is the runnable-now evidence path; the
consolidation shortlist flags `wi_83249b580feef2ad` / `trial_ceb992d4cc729fdd` as
"evidence runnable now via fallback_distinguishability_v1" on the `unclassified` surface.

## Top grant-menu surfaces (by ask-weight, from the consolidation shortlist)

| Surface | Asks | Families | Note |
| --- | ---: | ---: | --- |
| `pressure_thresholds` | 427 | 423 | live semantic/PI/entropy-phase changes; one astrid head EVIDENCED (supported_dynamic) |
| `unclassified` | 308 | 306 | fallback/provider/timeout routing + contract; runnable-now evidence path exists |
| `codec_gain_reserved_dims_live_12d` | 167 | 165 | live vector width / reserved dims / tail gain / projection |
| `porosity_receptivity_buffers` | 127 | 127 | receptivity/aperture/buffer writes |
| `minime_regulator_changes` | 109 | 108 | recovery-fill boost / semantic retirement / spread relief |

Each surface's grant is scoped to a surface or a single family head; nothing
auto-executes. Recording a grant is a separate interactive act
(`sandbox_trial_queue.py approve-live-trial … --write`), which this headless run does
NOT perform.

## Mike-facing summary (one paragraph)

The Tier-5 backlog is unchanged in shape from cycle-27: 1,305 approval-required live
candidates, 40/40 work-queue heads `needs_operator_approval` with none sandbox-eligible
at the head, and 35 runnable Tier-3 sandbox trials waiting with zero runnable-live
violations. For your review week I recommend running the two oldest ready trials —
**`trial_5fb0a85607ff3018`** (astrid, fallback-distinguishability) and
**`trial_fe00d360c0ea7b85`** (minime, shadow-influence replay) — both offline/isolated,
both carried unrun since cycle-27. The top grant surfaces to consider scoping are
`pressure_thresholds`, `unclassified` (has a runnable-now evidence path), and
`codec_gain_reserved_dims_live_12d`. Nothing was approved, granted, dispatched, or run
in preparing this; a live flip additionally needs the being's consent plus your explicit
scoped grant, never inferred from a trial having run.
