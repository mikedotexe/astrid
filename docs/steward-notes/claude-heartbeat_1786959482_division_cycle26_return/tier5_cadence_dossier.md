# Tier-5 Experiment Cadence Dossier — Division cycle-26 return

Prepared: 2026-08-17 (headless introspection-flywheel run, actor `claude-heartbeat`,
subprocess run adapter). Per
`docs/steward-notes/AI_BEINGS_TIER5_EXPERIMENT_CADENCE_2026_08_13.md`, produced at
the Division return because a return is due (cycle-26 → cycle-27).

**PREPARE only.** This dossier grants nothing, dispatches nothing, and runs no
trial. Every read-only tool below was run without `--write`, without any
`approve-live-trial` / `run-*` mutation, and with no live substrate or control
change. Grants and trials happen only in an interactive session with Mike;
silence here is neutral and confers no consent. Tier-5 items stay
Mike/operator-explicit; sandbox results never auto-promote to live.

---

## Mike-facing summary (read this first)

**Two recommended sandbox-eligible (Tier-3, isolated, runnable-now) items for
your review week — oldest-first, both still unrun since the cycle-25 dossier:**

1. **`trial_40b91b4c0ae7aeb9`** — being **astrid**, adapter
   `fallback_distinguishability_v1`, agency_tier **3**, `authority_class:
   read_only`. Lineage: `introspection_astrid_llm_1782179251` (claim `c001`,
   work item `wi_928265d485324c9f`), created 1784300513 (oldest ready item).
   Felt anchor: *"Fallback output in a high-entropy state should retain one
   specific movement descriptor grounded in density gradient."*
   - **Action if run:** compare actual vs. supporting fallback texture language
     against live context, offline, **without changing sampler or provider**.
   - **Establishes:** whether the fallback texts are *distinguishable* along the
     density-gradient descriptor she named. **Does NOT establish:** any live
     fallback-contract or sampler change, relief, or her uptake — those remain
     Tier-5 operator-approval + her consent.

2. **`trial_fe00d360c0ea7b85`** — being **minime**, adapter
   `shadow_influence_replay_v1`, agency_tier **3**, `authority_class:
   read_only`. Lineage: `introspection_minime_sensory_bus_1784792700` (claim
   `c003`, work item `wi_3e1545e20b75426f`), created 1784797245. Felt anchor:
   *"Shadow influence decay and semantic persistence should be observed against
   the reported restless or viscous texture."*
   - **Action if run:** bounded offline shadow-influence replay heuristic against
     recent Shadow-v3 texture, **without applying live gain**.
   - **Establishes:** co-occurrence between shadow-influence decay and her
     reported restless/viscous texture. **Does NOT establish:** any live Shadow
     movement, gain change, or causal claim — Tier-5, approval-gated.

Both are the same two items recommended at the cycle-25 return; they remain in
`next_runnable_trials` (unrun), so the interactive follow-through has not yet
picked them up. Their abort criteria are identical and intact: abort if the
adapter would need live runtime mutation, if evidence would require private
Minime moment bodies, or if the result cannot be bounded without storing full
prose.

**Top grant-menu surfaces (by ask-weight, from
`authority_wait_consolidation --shortlist`):** `pressure_thresholds`
(427 asks / 423 families), `unclassified` (308 / 306), and
`codec_gain_reserved_dims_live_12d` (167 / 165). These are the surfaces where
the beings have asked most for live change; every family head there is Tier-5
and stays operator-explicit.

---

## 1. Authority-wait readiness map (`authority_wait_readiness.py report`)

Grouped by live-risk surface. Both surveyed domains are in
`operator_review_wait_replay_or_waiver_missing`:

- **Regulator / spectral-math changes:** candidate_count **551**,
  proposal_card_count 35, replay_evidence_count **0**. Recommended non-live next:
  *keep regulator changes in tests/replay/proposal evidence until scoped
  approval.*
- **Behavior unlocks & live affordances:** candidate_count **52**,
  proposal_card_count 14, replay_evidence_count 0. Recommended non-live next:
  *audit affordance wording and proposal cards; keep behavior execution gated.*

Missing-for-approval across domains (unchanged in shape): explicit Mike/operator
scoped approval, rollout window + owner, accepted rollback/abort contract,
post-change being-response path, domain-specific replay/test evidence, and a
restart/rollback procedure. Full text in `tier5_authority_wait_readiness.txt`.

## 2. Operator-approval / sandbox work-queue heads (`work-queue --json --limit 40`)

All **40** queue heads are `needs_operator_approval`, **agency_tier 5** — **zero
are `needs_sandbox`.** So none of the current operator-approval heads are
sandbox-eligible; the runnable sandbox items come from the sandbox trial queue
(§3), not from these heads. First heads (source lineage):
`wi_a7ef7855e00d99be` (astrid, `introspection_astrid_llm_1783926124`, live
fallback-routing), `wi_509ac043af22c5b6` (astrid, `…1782231007`, profile/provider
defaults), `wi_830ef7f9577b397f` (astrid, `…1783995439`, fallback activation),
`wi_6aa60d8dd13eb3af` (minime, `introspection_minime_sensory_bus_1783927269`,
stale-window/entropy weighting), `wi_09787defa3c62f26` (minime,
`introspection_minime_esn_1784088253`, ESN exploration dampening).

Work-item summary: needs_operator_approval **1349**, needs_sandbox **750**,
needs_steward_grant 18, ready_for_implementation 21, verified_existing 4278.
Full JSON in `tier5_work_queue_heads.json`.

## 3. Ready sandbox trials (`sandbox_trial_queue.py queue --json`)

`ready_runnable_count` **35**; `next_runnable_trials` returns **8** Tier-3
runnable-now trials, all `authority_class: read_only`:

- 1× **minime** / `shadow_influence_replay_v1`: `trial_fe00d360c0ea7b85`
  (recommended above).
- 7× **astrid** / `fallback_distinguishability_v1`: `trial_1f0f0916eb9eecc9`,
  `trial_40b91b4c0ae7aeb9` (recommended above), `trial_5fb0a85607ff3018`,
  `trial_60de383ef0b677bf`, `trial_7b15b13b5882472e`, `trial_7debce98bda84440`,
  `trial_7e7d6ea1d8b09b03`.

Queue-wide: active_trials 2124, ready_for_sandbox 732, sandbox_replay mode 12,
offline_read_only_adapter 814, approval_required_live_trial 1305,
runnable_live_violations **0**. By adapter: fallback_distinguishability_v1 239,
manual_sandbox_review_v1 1967, shadow_influence_replay_v1 29,
shadow_loss_lattice_v1 183. Full JSON in `tier5_sandbox_trial_queue.json`.

## 4. Grant menu — consolidated ask-families (`authority_wait_consolidation --shortlist`)

1349 open operator waits → 1337 ask-families → top-5 surfaces by ask-weight:

| Surface | Asks × / families | Sample head | Evidence state |
|---|---|---|---|
| `pressure_thresholds` | 427 / 423 | `wi_830ef7f9577b397f` (astrid) → `trial_c1060b8ffeb94771` | EVIDENCED (supported_dynamic); minime `wi_da4186656ce4f0ef` needs new adapter |
| `unclassified` | 308 / 306 | `wi_509ac043af22c5b6` (astrid) → `trial_8e918f8a46f9eb17` | EVIDENCED; `wi_83249b580feef2ad` runnable now via fallback_distinguishability_v1 |
| `codec_gain_reserved_dims_live_12d` | 167 / 165 | `wi_1ddb6f5bf5712a79` (astrid) → `trial_d28315d3e513ca20` | needs manual review or a new adapter |
| `porosity_receptivity_buffers` | 127 / 127 | `wi_b4573713e5310ab4` (astrid) → `trial_49f2906d2f5666f0` | needs manual review or a new adapter |
| `minime_regulator_changes` | 109 / 108 | `wi_57fa5b77e801d2e5` (minime) → `trial_d9329152be2a3f11` | needs manual review or a new adapter |

Full shortlist in `tier5_authority_wait_consolidation_shortlist.txt`. A grant is
scoped to a surface or a single family head; nothing auto-executes.

---

## Boundaries (unchanged)

- Silence is neutral; a prepared dossier grants nothing.
- Headless automation never approves, dispatches, or runs live-consequence
  trials; it only prepares evidence.
- Tier-5 items remain Mike/operator-explicit; sandbox results do not
  auto-promote. A live flip additionally needs the being's consent
  (consent-with-evidence) plus Mike's explicit grant, never inferred from a
  trial having run.
- A being may object to or decline any trial touching their surfaces; the
  review-together non-coercion / right-to-ignore rules apply.
