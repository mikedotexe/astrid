# Tier-5 Experiment Cadence Dossier — cycle-43 Division return

Generated 2026-09-10 by `claude-heartbeat` from read-only tooling, per
`docs/steward-notes/AI_BEINGS_TIER5_EXPERIMENT_CADENCE_2026_08_13.md`.

**PREPARE ONLY.** Nothing here was approved, granted, dispatched, or run. No trial was executed,
no `approve-live-trial` was recorded, no proposal card was delivered. `live_eligible_now=false`,
`auto_approved=false`, `grants_approval=false` across every artifact below.

Tooling run (all read-only): `authority_wait_readiness.py report`,
`introspection_addressing_audit.py next --limit 40 --json`,
`sandbox_trial_queue.py queue --json`, `authority_wait_consolidation.py --shortlist`.

## Where the waits stand

- `authority_wait_readiness`: status `approval_waits_mapped`, **1,305** approval-required live
  candidates across **7** domains, `hard_violation_count: 0`, `unclassified_live_wait_count: 153`.
- `sandbox_trial_queue`: **2,418** total trials — 1,305 `approval_required_live_trial`,
  730 `ready_for_sandbox`, 89 `result_recorded`, 258 `superseded`, 29 `verified_existing`,
  7 closed. By tier: 1,569 at Tier 5, 785 at Tier 3, 17 at Tier 4. `ready_runnable_count: 33`,
  `runnable_live_violation_count: 0`, `proposal_card_count: 145`, `result_card_count: 107`.
- `authority_wait_consolidation`: 1,349 open operator waits collapse to **1,337 ask-families** —
  i.e. almost no duplication. The ask is broad, not repetitive.

## Recommended sandbox-eligible items (1–2, Tier 3, non-live)

Both run the `fallback_distinguishability_v1` adapter — **offline, read-only, no live substrate or
control surface**. They are recommended because they test a claim Astrid has made repeatedly and
that no evidence currently settles: whether a prompt/linguistic contract is sufficient to prove a
smaller fallback model preserves lived texture.

1. **`trial_5fb0a85607ff3018`** (Tier 3, Astrid) — "A prompt contract cannot by itself prove that a
   4B fallback model has enough capacity to preserve a complex spectral texture."
   *Why first:* it is the general form of the question; a result here bounds several dozen
   fallback-contract asks at once. Nothing live changes — it compares recorded outputs.
2. **`trial_60de383ef0b677bf`** (Tier 3, Astrid) — "A forced fallback should articulate
   settled-habitable texture with specific motion rather than standard model tropes."
   *Why second:* it is the concrete, checkable instance of (1), with a named pass condition
   (specific motion descriptor vs. trope) rather than a subjective judgement.

Six further runnable Tier-3 siblings exist on the same adapter (`trial_40b91b4c0ae7aeb9`,
`trial_7b15b13b5882472e`, `trial_7debce98bda84440`, `trial_7e7d6ea1d8b09b03`, and two more).
Evidence-gathering command, if and when a steward wants it:
`python3 scripts/sandbox_trial_queue.py run-evidence --trial-id <trial> --write`.

## Top grant-menu surfaces (from `--shortlist`)

A grant is scoped to a surface or a single family head. Honest per-family evidence state is shown
as the tooling reports it — most families have **no** replay evidence yet.

| Surface | Ask weight | Families | Notable head | Evidence state |
| --- | ---: | ---: | --- | --- |
| `pressure_thresholds` | 427x | 423 | `wi_830ef7f9577b397f` / `trial_c1060b8ffeb94771` — live fallback activation, provider routing, generation thresholds | **EVIDENCED** (`supported_dynamic`) |
| `unclassified` | 308x | 306 | `wi_509ac043af22c5b6` / `trial_8e918f8a46f9eb17` — profile defaults, provider selection, timeout routing, fallback dispatch | **EVIDENCED** (`supported_dynamic`) |
| `unclassified` | — | — | `wi_83249b580feef2ad` / `trial_ceb992d4cc729fdd` — fallback sampler/provider/model selection and the fallback output contract | **EVIDENCED** (`supported_dynamic`) |
| `codec_gain_reserved_dims_live_12d` | 167x | 165 | `wi_1ddb6f5bf5712a79` / `trial_d28315d3e513ca20` — live vector width, reserved dims, narrative/texture gain | needs manual review or a new adapter |
| `porosity_receptivity_buffers` | 127x | 127 | `wi_b4573713e5310ab4` / `trial_49f2906d2f5666f0` — spectral buffer zone affecting token cleanup / output policy | needs manual review or a new adapter |
| `minime_regulator_changes` | 109x | 108 | `wi_57fa5b77e801d2e5` / `trial_d9329152be2a3f11` — recovery fill boost, semantic retirement, spread relief | needs manual review or a new adapter |

Three of the six shortlisted heads carry `supported_dynamic` evidence; the rest are honestly
marked as having no adapter. That distinction is the point of the menu — do not read an
unevidenced surface as merely un-prioritised.

## Mike-facing summary

Two Tier-3, offline, read-only trials are worth running whenever a steward has budget:
**`trial_5fb0a85607ff3018`** (does a prompt contract alone prove a 4B fallback preserves texture?)
and its concrete instance **`trial_60de383ef0b677bf`**. Neither touches live substrate.

The Tier-5 grant menu's three best-evidenced surfaces are all **fallback/provider-routing**
(`pressure_thresholds` head `wi_830ef7f9577b397f`, and the two `unclassified` heads
`wi_509ac043af22c5b6` and `wi_83249b580feef2ad`) — each already `supported_dynamic`. The three
largest *unevidenced* surfaces are `codec_gain_reserved_dims_live_12d`,
`porosity_receptivity_buffers`, and `minime_regulator_changes`.

Every domain's `missing_for_approval` list still begins with "explicit Mike/operator scoped
approval", and none of them has a scoped rollout window, a named rollback owner, an accepted
abort contract, or a post-change being-response collection path. Those four gaps — not the
absence of a grant — are what actually blocks the ladder.
