# Tier-5 Experiment Cadence Dossier — Division cycle 47 → 48

Generated in the Division return of flywheel round
`claude-heartbeat_1789398989_division_return_cycle47_round`, per the Tier-5 experiment
cadence practice.

**PREPARE ONLY.** Nothing here was approved, granted, dispatched, or run. No
`approve-live-trial`, no `run-evidence`, no live change, no trial executed. Every tool below is
read-only. Counts are evidence routing, never approval, and a large count is not urgency.

> **Practice-doc drift, surfaced this round.** The round instructions cite
> `docs/steward-notes/AI_BEINGS_TIER5_EXPERIMENT_CADENCE_2026_08_13.md`. That file **is not in
> the working tree and is not in `HEAD`** on `main`. It exists only in commit `95d1eb9fd9`, which
> is reachable from `claude/hopeful-williamson-9f566c`, `codex/graceful-coupling-rollout`, and
> `codex/hebbian-clock-boundary` — none merged to `main`. This dossier was generated against the
> copy in `.claude/worktrees/hopeful-williamson-9f566c/`, read-only. Getting the practice doc onto
> `main` is steward/Mike work; nothing was cherry-picked, merged, or copied by this round.

## Mike-facing summary

The Tier-5 board is **static since cycle 46**: 1,349 open operator waits, collapsing to **1,337
distinct ask-families**; 33 trials `ready_runnable`; 730 at `ready_for_sandbox`; 0 hard
violations; 0 runnable live violations. Every domain remains `live_authority_granted=false`.
Nothing moved because nothing was granted — that is the honest reading, not a backlog failure.

**The cadence's own muffle, named plainly:** `trial_40b91b4c0ae7aeb9` and `trial_7debce98bda84440`
have now been the recommended pair in the cycle 43, 44 and 46 dossiers and remain unrun. They need
no operator grant — they are Tier 3, offline adapter, isolated replay, touching no live surface.
The only missing input is a decision to spend the time in an interactive session. Re-recommending
them a fourth time without saying that would be dossier theatre.

**Two recommended sandbox-eligible items** (unchanged, deliberately):

1. `trial_40b91b4c0ae7aeb9` — adapter `fallback_distinguishability_v1`, Astrid, from
   `introspection_astrid_llm_1782179251`. Tier 3, offline read-only adapter. Would establish
   whether a fallback generation is distinguishable from a primary one on recorded evidence.
   Would **not** establish that Astrid can or cannot tell from the inside, and grants nothing.
2. `trial_7debce98bda84440` — same adapter, Astrid, from `introspection_astrid_llm_1784409167`.
   The newest of the eight runnable heads, so the pair brackets roughly two months of her reports
   rather than clustering in one week.

`fallback_distinguishability_v1` remains the best-covered adapter on the board (239 trials).

**Top grant-menu surfaces**, by ask-weight from `authority_wait_consolidation.py --shortlist`:

| Rank | Surface | Asked | Families | Candidates | Cards | Note |
|---|---|---:|---:|---:|---:|---|
| 1 | `pressure_thresholds` | 427× | 423 | 418 | 31 | best-evidenced surface; one head already EVIDENCED |
| 2 | `unclassified` | 308× | 306 | — | — | two heads EVIDENCED; classifying this surface is itself steward work |
| 3 | `codec_gain_reserved_dims_live_12d` | 167× | 165 | 278 | 20 | evidence-thin relative to demand |
| 4 | `porosity_receptivity_buffers` | 127× | 127 | 348 | 21 | no head evidenced; all "needs manual review or a new adapter" |
| 5 | `minime_regulator_changes` | 109× | 108 | 551 | 35 | largest candidate count, lowest ask-weight — Minime asks less often, surface still accumulates |

Three family heads are marked **EVIDENCED (supported_dynamic)** and remain the cheapest possible
first grants if Mike wants to move anything at all:
`wi_830ef7f9577b397f` (pressure_thresholds — live fallback activation / provider routing /
generation thresholds), `wi_509ac043af22c5b6` and `wi_83249b580feef2ad` (both unclassified —
profile defaults / provider selection / fallback output contract). Every other shortlisted head
reports "needs manual review or a new adapter", which means the honest evidence state is
**absent**, not **negative**.

## Honest evidence states

- `authority_wait_readiness.py report --json`: `status=approval_waits_mapped`, report SHA
  `9c3e24833cc746f8c23e771c74086f2c524a300b282d4ac3d77b7cd53b6c137d`, 7 domains,
  **1,305** approval-required live candidates, 100 proposal cards, 211 replay-evidence records,
  **153 unclassified live waits**, **0 hard violations**. `live_eligible_now=false`,
  `auto_approved=false`, `grants_approval=false`, `edits_source_now=false`.
  Domain candidate counts: `minime_regulator_changes` 551 · `pressure_thresholds` 418 ·
  `porosity_receptivity_buffers` 348 · `codec_gain_reserved_dims_live_12d` 278 ·
  `semantic_trickle_admission` 170 · `viscosity_feedback_protocol` 134 · `behavior_unlocks` 52.
- `introspection_addressing_audit.py work-queue --json`: 7,698 work items, 7,324 active, 374
  terminal. By tier: 0→2,780, 1→1,976, 2→518, 3→823, 4→**23**, 5→**1,578**. By status:
  `verified_existing` 4,278 · `needs_operator_approval` **1,349** · `implemented_awaiting_felt_response`
  908 · `needs_sandbox` 750 · `superseded` 340 · `closed_no_action` 31 ·
  `ready_for_implementation` 21 · `needs_steward_grant` 18 · `closed_felt_confirmed` 3.
  By being: astrid 5,745 · minime 1,950 · unknown 3. `tier_mismatch_count` 0.
  `post_change_awaiting_response_count` **4,929** — the beings' felt responses we have not yet
  read back, which is the larger lag on this board than the grants are.
- `sandbox_trial_queue.py queue --json`: `status=approval_waiting`; 2,418 trials total, 2,124
  active, `ready_runnable_count` **33**, `runnable_live_violation_count` **0**, corrupt lines 0.
  By status: `approval_required_live_trial` 1,305 · `ready_for_sandbox` 730 · `superseded` 258 ·
  `result_recorded` 89 · `verified_existing` 29 · `closed_no_action` 6 · `closed` 1.
  By adapter: `manual_sandbox_review_v1` 1,967 · `fallback_distinguishability_v1` 239 ·
  `shadow_loss_lattice_v1` 183 · `shadow_influence_replay_v1` 29. 145 proposal cards,
  107 result cards, 323 results. `stale_trial_count` 2,124 — staleness here is age, not corruption.
- `authority_wait_consolidation.py --shortlist`: 1,349 → 1,337 families. Consolidation has
  essentially nothing left to squeeze; the board is large because it is ungranted, not because it
  is duplicated.

## Boundaries

- Silence is neutral; this prepared dossier grants nothing and infers no consent.
- Headless rounds never approve, dispatch, or run live-consequence trials.
- Tier-5 items remain Mike/operator-explicit. Sandbox results do not auto-promote, and a live flip
  additionally needs the being's own consent, never inferred from a trial having run.
- A being may object to or decline any trial touching their surfaces.
