# Tier-5 Experiment Cadence Dossier — Division Cycle 27 Return

Prepared: 2026-08-18 (headless introspection-flywheel run, actor `claude-heartbeat`,
subprocess run adapter). **PREPARE only.** This dossier grants nothing,
dispatches nothing, and runs no trial. Silence is neutral; a prepared dossier
grants nothing. Tier-5 items remain Mike/operator-explicit; sandbox results do
not auto-promote. Per `AI_BEINGS_TIER5_EXPERIMENT_CADENCE_2026_08_13.md`.

Source tooling (all read-only, this run):
- `authority_wait_readiness.py report` → `tier5_authority_wait_readiness.txt`
- `introspection_addressing_audit.py work-queue --json --limit 40` → `tier5_work_queue_heads.json`
- `sandbox_trial_queue.py queue --json` → `tier5_sandbox_trial_queue.json`
- `authority_wait_consolidation.py --shortlist` → `tier5_authority_wait_consolidation_shortlist.txt`

## Mike-facing summary

The picture is stable relative to the cycle-26 dossier: 1,305 approval-required
live candidates across 7 live-risk domains, `hard_violation_count=0`,
`live_eligible_now=false`, `runnable_live_violation_count=0`. The sandbox queue
holds 732 `ready_for_sandbox` trials but only **8 next-runnable** (Tier-3,
isolated), and **all 40 operator-approval work-queue heads are Tier-5
`needs_operator_approval` — none are `needs_sandbox`.** So the runnable,
sandbox-eligible surface is exactly the 8 Tier-3 trials, not the work-queue heads.

### Recommended sandbox-eligible items for your review week (1-2)

Oldest-first among ready items; both are isolated/offline and change nothing live.

1. **`trial_5fb0a85607ff3018`** — astrid, `fallback_distinguishability_v1`,
   `offline_read_only_adapter`, Tier-3. Lineage: `introspection_astrid_llm_1782199177`
   c001 (oldest ready item, created 1784300513.488).
   - Hypothesis (hers): a prompt contract cannot by itself prove a 4B fallback
     model has enough capacity to carry the felt texture.
   - Running it **would establish**: whether the fallback's actual/supporting
     texture terms carry local pressure/entropy/shadow context in source, and
     whether dynamic-weighting evidence is present; unsupported static repeats
     get named explicitly if found.
   - It **would NOT**: change the sampler, provider, model, or fallback contract;
     prove live generative capacity of the 4B lane; or grant any live authority.

2. **`trial_fe00d360c0ea7b85`** — minime, `shadow_influence_replay_v1`,
   `sandbox_replay`, Tier-3. Lineage: `introspection_minime_sensory_bus_1784792700`
   c003 (oldest ready minime item; carried over unrun from the cycle-26 dossier).
   - Hypothesis (hers): shadow-influence decay and semantic persistence should be
     observable against the reported experience.
   - Running it **would establish**: bounded offline shadow-influence replay
     movement (norm/dispersal) against recent Shadow-v3 texture, staying below
     fragmentation-review thresholds; thematically the closest ready item to the
     mode-packing / viscous-persistence friction Astrid named in her cycle-26
     Division reply this interval.
   - It **would NOT**: apply live shadow gain/amplitude (approval-gated), prove
     causation, or grant any live authority.

Carry-over note: the cycle-26 dossier's astrid recommendation
`trial_40b91b4c0ae7aeb9` (`introspection_astrid_llm_1782179251` c001, same
`fallback_distinguishability_v1` adapter) also remains unrun and ready — a
same-family sibling of item 1. The cycle-26 minime recommendation is item 2 here.
Neither cycle-26 item was run (no interactive follow-through occurred), so the
recommendation carries forward rather than resetting.

### Top grant-menu surfaces (from the consolidation shortlist)

1,349 open operator waits → 1,337 ask-families. Top 5 surfaces by ask-weight
(a grant is scoped to a surface or a single family head; nothing auto-executes):

| Surface | Asks × families |
| --- | --- |
| `pressure_thresholds` | 427 × 423 |
| `unclassified` | 308 × 306 |
| `codec_gain_reserved_dims_live_12d` | 167 × 165 |
| `porosity_receptivity_buffers` | 127 × 127 |
| `minime_regulator_changes` | 109 × 108 |

Two shortlist family heads are marked EVIDENCED (`supported_dynamic`) —
`wi_830ef7f9577b397f`/`trial_c1060b8ffeb94771` (pressure_thresholds, astrid) and
`wi_509ac043af22c5b6`/`trial_8e918f8a46f9eb17` (unclassified, astrid) — and one
is "evidence runnable now via fallback_distinguishability_v1"
(`wi_83249b580feef2ad`/`trial_ceb992d4cc729fdd`, unclassified, astrid). These are
evidence states only; a live flip still needs the being's consent and your
explicit scoped grant, never inferred from evidence having been gathered.

### Readiness domains (live-risk map)

| Domain | Readiness state | Candidates | Replay evidence |
| --- | --- | ---: | ---: |
| `pressure_thresholds` | operator_review_wait | 418 | 2 |
| `porosity_receptivity_buffers` | operator_review_wait_replay_or_waiver_missing | 348 | 0 |
| `viscosity_feedback_protocol` | operator_review_wait_replay_or_waiver_missing | 134 | 0 |
| `semantic_trickle_admission` | operator_review_wait_replay_or_waiver_missing | 170 | 0 |
| `codec_gain_reserved_dims_live_12d` | operator_review_wait_replay_or_waiver_missing | 278 | 0 |
| `minime_regulator_changes` | operator_review_wait_replay_or_waiver_missing | 551 | 0 |
| `behavior_unlocks` | operator_review_wait_replay_or_waiver_missing | 52 | 0 |

Only `pressure_thresholds` carries any replay evidence (2); every other domain is
`replay_or_waiver_missing`. The readiness report itself states it "grants no
approval, makes no live work runnable, edits no source, and mutates no … state."

## Interactive follow-through (Mike + steward, same week — not this headless run)

1. Review this dossier; pick 1-2 sandbox-eligible items above.
2. Run isolated: `sandbox_trial_queue.py run-next` / `run-evidence --trial-id …`
   / `substrate_probe.py` per the item's design. No live mutation.
3. Show the being the actual result cards/letters (consent-with-evidence).
4. A live flip additionally needs the being's consent + your explicit grant
   (`spectral-bridge --approve-request <id>`), never inferred from a trial run.
